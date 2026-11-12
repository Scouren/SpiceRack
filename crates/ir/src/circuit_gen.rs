//! Random valid circuit rows for the generator: a `Gen`, a scope and a feature set in, one
//! pushed `Element` or `Model` out, valid under `check` and using only features in the set.
//! Every optional field is set only when its `CircuitExtra` is in the set, every kind or law
//! only when its feature is.
//!
//! Order is part of the contract: names are interned and expressions created in exactly the
//! order `dump` writes them (header, then fields in declaration order, a side or wave row's
//! fields at its field's position), so `parse(dump(d))` rebuilds equal tables. Rows are pushed
//! through the asserting builder, so a generator bug is an assertion, not a bad design.
//!
//! Cites: `rewrite/IR-IMPL.md` §2 (generator, round trip); `rewrite/IR-KERNEL.md` §9; the
//! IR-text forms of `circuit_text.rs`.
//!
//! Contents (crate-private): `pub(crate) fn generate_element`, `pub(crate) fn generate_model`,
//! `pub(crate) fn generate_library`;
//! the row bodies of `R` are in `circuit_gen_rows.rs`, the models in `circuit_gen_model.rs`
//! (child modules: they read `R`'s private helpers).

use crate::build::Builder;
use crate::build_circuit::ElemHead;
use crate::circuit_check::{builtin_kind, master_ports};
use crate::circuit_extra::CircuitExtra as X;
use crate::controlled::{
    Controlled, Delay, DelayLaw, FreqLaw, FreqTable, FreqUnit, Laplace, LaplaceForm, Poly,
    Quantity, TableLaw, TableSource,
};
use crate::element::{DetailKind, ElemKind, MultLaw, NO_ROW};
use crate::feature::{Feature, FeatureSet};
use crate::r#gen::placeable;
use crate::r#gen::{Gen, Reads};
use crate::id::{DefId, ExprId, ExprList, ItemRef, List, NameId, Ref, ScopeId};
use crate::item::{DefKind, ItemKind};
use crate::letter::letter_of;
use crate::model::ModelFamily;
use crate::passive::{Coupling, IcMode};
use crate::semi::{InitialState, Semi, Switch};
use crate::span::Span;

#[path = "circuit_gen_head.rs"]
mod head;
#[path = "circuit_gen_model.rs"]
mod model;
#[path = "circuit_gen_rows.rs"]
mod rows;
#[path = "circuit_gen_waves.rs"]
mod waves;

pub(crate) use rows::generate_library;

const N: ExprId = ExprId::NONE;
const NONE_REF: Ref = Ref {
    name: NameId::NONE,
    def: DefId::NONE,
};

/// The builtin families and the kinds they serve (AMEND-A12 C5).
const BUILTINS: [(ModelFamily, ElemKind); 7] = [
    (ModelFamily::SourceV, ElemKind::VSource),
    (ModelFamily::SourceI, ElemKind::ISource),
    (ModelFamily::Vcvs, ElemKind::Vcvs),
    (ModelFamily::Vccs, ElemKind::Vccs),
    (ModelFamily::Ccvs, ElemKind::Ccvs),
    (ModelFamily::Cccs, ElemKind::Cccs),
    (ModelFamily::TlineIdeal, ElemKind::LosslessLine),
];

/// The generator's state for one row.
struct R<'r, 'b> {
    g: &'r mut Gen,
    b: &'r mut Builder<'b>,
    f: &'r FeatureSet,
    scope: ScopeId,
}

/// Pushes one random element into `scope`, or returns `ItemRef::NONE` when the set allows no
/// element kind this generator can build.
pub(crate) fn generate_element(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    features: &FeatureSet,
) -> ItemRef {
    let mut r = R {
        g,
        b,
        f: features,
        scope,
    };
    let kinds: Vec<ElemKind> = ElemKind::ALL.into_iter().filter(|&k| r.can(k)).collect();
    if kinds.is_empty() {
        return ItemRef::NONE;
    }
    let kind = kinds[r.g.below(kinds.len() as u32) as usize];
    // R48 (d): an element that names a model mostly names one the design defines, so a model
    // of a family the kind takes is drawn first when none is visible (before the element
    // interns a name: `dump` writes the model's line first)
    if kind.master_kind() == Some(DefKind::Model)
        && r.f.contains(Feature::Item(ItemKind::Model))
        && placeable(r.b.design(), scope, ItemKind::Model, r.f)
        && r.fitting_model(kind).is_none()
    {
        let builtin_ok = r.has(X::BuiltinMaster);
        let fits: Vec<ModelFamily> = (r.model_families().into_iter())
            .filter(|&f| head::takes(kind, f) && (builtin_kind(f).is_none() || builtin_ok))
            .collect();
        // ponytail: the model does not count against the item budget (as a required title)
        if !fits.is_empty() && r.g.below(8) != 0 {
            r.push_model(&fits);
        }
    }
    // decisions that fix the terminal count, before anything is created
    let two_terminal = match kind {
        ElemKind::Laplace | ElemKind::FreqTable => r.maybe(X::LaplaceExprInput),
        ElemKind::VSwitch => r.maybe(X::SwitchControlExpr),
        _ => false,
    };
    // the output and control quantities fix the SPICE letter, so they come before the name
    let out = if r.g.chance(2) {
        Quantity::Voltage
    } else {
        Quantity::Current
    };
    let ctl = if r.g.chance(2) {
        Quantity::Voltage
    } else {
        Quantity::Current
    };
    let name = match letter_of(kind, out, ctl) {
        Some(l) if !r.has(X::NameWithoutLetter) || r.g.chance(2) => {
            let text = format!("{}{}", char::from(l.to_ascii_lowercase()), r.g.fresh());
            r.b.name(&text)
        }
        _ => r.g.fresh_name(r.b),
    };
    // R61/A36: an instance's terminals follow its subcircuit's ports, so its master is picked
    // first (no name is interned for it: a bound master's name already is)
    let bound = (kind == ElemKind::Instance)
        .then(|| r.bound_subckt())
        .flatten();
    let ports = bound.and_then(|m| master_ports(r.b.design(), m));
    let terminals = r.terminals(kind, two_terminal, ports);
    let model = match bound {
        Some(m) => m,
        None => r.master(kind),
    };
    // an instance's multiplier comes with its law (A15 C1)
    let mult_law =
        if kind == ElemKind::Instance && r.has(X::MultInstanceNgspiceAppend) && r.g.chance(2) {
            MultLaw::NgspiceAppend
        } else {
            MultLaw::Parallel
        };
    let mult_feature = match mult_law {
        MultLaw::NgspiceAppend => X::MultInstanceNgspiceAppend,
        MultLaw::Parallel => crate::circuit_features::mult_class(kind),
    };
    let mult = if mult_law == MultLaw::NgspiceAppend || r.maybe(mult_feature) {
        r.ex()
    } else {
        ExprId::NONE
    };
    let head = ElemHead {
        name,
        terminals,
        model,
        params: List::EMPTY,
        mult,
        mult_law,
    };
    let detail = r.detail(kind, two_terminal, out, ctl);
    r.b.element(scope, kind, head, detail, Span::NONE)
}

/// Pushes one random model into `scope` (always possible).
pub(crate) fn generate_model(
    g: &mut Gen,
    scope: ScopeId,
    b: &mut Builder<'_>,
    features: &FeatureSet,
) -> ItemRef {
    let mut r = R {
        g,
        b,
        f: features,
        scope,
    };
    let families = r.model_families();
    if families.is_empty() {
        return ItemRef::NONE;
    }
    r.push_model(&families)
}

// ---- private ----

impl R<'_, '_> {
    fn has(&self, x: X) -> bool {
        self.f.contains(Feature::Circuit(x))
    }

    /// True with probability ½ when `x` is in the set, else false.
    fn maybe(&mut self, x: X) -> bool {
        self.has(x) && self.g.chance(2)
    }

    /// Whether `kind` is in the set and every law it must pick has a member in the set.
    fn can(&self, kind: ElemKind) -> bool {
        self.f.contains(Feature::Elem(kind))
            && match kind {
                ElemKind::Table => {
                    self.has(X::TableExact)
                        || self.has(X::TableNgspiceRounded)
                        || self.has(X::TableSmoothed)
                }
                ElemKind::FreqTable => self.has(X::FreqComplexLinear) || self.has(X::FreqDbLogFreq),
                _ => true,
            }
            && (self.has(X::ElementWithoutModel)
                || kind == ElemKind::Unresolved
                || match kind.master_kind() {
                    Some(_) if kind == ElemKind::Coupling => {
                        self.has(X::CouplingCore)
                            || self.has(X::BuiltinMaster) && self.has_visible(ModelFamily::Mutual)
                    }
                    Some(_) if BUILTINS.iter().any(|b| b.1 == kind) => self.has(X::BuiltinMaster),
                    Some(_) => true,
                    // names no master, so it needs no `ElementWithoutModel`
                    None => true,
                })
    }

    fn ex(&mut self) -> ExprId {
        self.g.expr(self.b, self.f, Reads::Constant)
    }

    fn dy(&mut self) -> ExprId {
        self.g.expr(self.b, self.f, Reads::Circuit)
    }

    /// An expression when `x` is drawn, else `NONE`.
    fn opt(&mut self, x: X) -> ExprId {
        if self.maybe(x) { self.ex() } else { N }
    }

    fn exprs(&mut self, count: u32) -> ExprList {
        let ids: Vec<ExprId> = (0..count).map(|_| self.ex()).collect();
        if ids.is_empty() {
            List::EMPTY
        } else {
            self.b.exprs(&ids)
        }
    }

    /// One of `choices` (members whose feature is given as `Some(x)` only when `x` is in the
    /// set). Asserts at least one is allowed.
    fn pick<T: Copy>(&mut self, choices: &[(T, Option<X>)]) -> T {
        let allowed: Vec<T> = choices
            .iter()
            .filter(|(_, x)| x.is_none_or(|x| self.has(x)))
            .map(|c| c.0)
            .collect();
        assert!(!allowed.is_empty(), "no allowed member");
        allowed[self.g.below(allowed.len() as u32) as usize]
    }

    /// Pushes `kind`'s detail row (its side and wave rows first, at their field positions).
    fn detail(&mut self, kind: ElemKind, two: bool, out: Quantity, ctl: Quantity) -> u32 {
        match kind.detail() {
            DetailKind::None => NO_ROW,
            DetailKind::Passive => {
                let row = self.passive(kind);
                self.b.detail(row)
            }
            DetailKind::Coupling => {
                let count = if self.maybe(X::CouplingMany) { 3 } else { 2 };
                let inductors = self.element_names(count);
                let k = self.dy();
                self.b.detail(Coupling { inductors, k })
            }
            DetailKind::Source => {
                let row = self.source(kind);
                self.b.detail(row)
            }
            DetailKind::Controlled => {
                let gain = self.ex();
                let sense = if matches!(kind, ElemKind::Cccs | ElemKind::Ccvs) {
                    self.element_name()
                } else {
                    NameId::NONE
                };
                self.b.detail(Controlled { gain, sense })
            }
            DetailKind::Poly => {
                let dimension = 1 + self.g.below(2) as u8;
                let control = ctl;
                let per = if control == Quantity::Voltage { 2 } else { 1 };
                let controls = if control == Quantity::Current {
                    self.element_names(u32::from(dimension))
                } else {
                    self.fresh_names(per * u32::from(dimension))
                };
                let n = 1 + self.g.below(3);
                let coeffs = self.exprs(n);
                let output = out;
                self.b.detail(Poly {
                    controls,
                    coeffs,
                    dimension,
                    output,
                    control,
                })
            }
            DetailKind::Table => {
                let law = self.pick(&[
                    (TableLaw::Exact, Some(X::TableExact)),
                    (TableLaw::NgspiceRounded, Some(X::TableNgspiceRounded)),
                    (TableLaw::Smoothed, Some(X::TableSmoothed)),
                ]);
                let input = self.dy();
                let n = 2 + 2 * self.g.below(2);
                let points = self.exprs(n);
                let delta = if law == TableLaw::Smoothed && self.g.chance(2) {
                    self.ex()
                } else {
                    N
                };
                let output = out;
                self.b.detail(TableSource {
                    input,
                    points,
                    delta,
                    law,
                    output,
                })
            }
            DetailKind::Behav => {
                let row = self.behav(kind);
                self.b.detail(row)
            }
            DetailKind::Laplace => {
                let form = self.pick(&[
                    (LaplaceForm::Polynomial, None),
                    (LaplaceForm::PoleZero, Some(X::LaplacePoleZero)),
                ]);
                let (a, c) = (1 + self.g.below(3), 1 + self.g.below(3));
                let num = self.exprs(a);
                let den = self.exprs(c);
                let input = if two { self.dy() } else { N };
                let output = out;
                self.b.detail(Laplace {
                    num,
                    den,
                    input,
                    output,
                    form,
                })
            }
            DetailKind::FreqTable => {
                let law = self.pick(&[
                    (FreqLaw::ComplexLinear, Some(X::FreqComplexLinear)),
                    (FreqLaw::DbLogFreq, Some(X::FreqDbLogFreq)),
                ]);
                let input = if two { self.dy() } else { N };
                let n = 3 + 3 * self.g.below(2);
                let points = self.exprs(n);
                let unit = FreqUnit::ALL[self.g.below(3) as usize];
                let output = out;
                self.b.detail(FreqTable {
                    input,
                    points,
                    law,
                    unit,
                    output,
                })
            }
            DetailKind::Delay => {
                let law = if self.maybe(X::DelaySampled) {
                    DelayLaw::Sampled
                } else {
                    DelayLaw::Ideal
                };
                let td = self.ex();
                let gain = self.ex();
                let samples = if law == DelayLaw::Sampled {
                    self.ex()
                } else {
                    N
                };
                self.b.detail(Delay {
                    td,
                    gain,
                    samples,
                    law,
                })
            }
            DetailKind::Semi => {
                let area = self.opt(X::SemiArea);
                let (ic, ic_mode) = self.ic_list();
                let temp_abs = self.opt(X::InstanceTemp);
                let dtemp = self.opt(X::InstanceDtemp);
                let initial = self.pick(&[
                    (InitialState::Unset, None),
                    (InitialState::Off, Some(X::SemiInitialOff)),
                    (InitialState::On, Some(X::SemiInitialOn)),
                ]);
                self.b.detail(Semi {
                    area,
                    ic,
                    temp_abs,
                    dtemp,
                    ic_mode,
                    initial,
                })
            }
            DetailKind::Switch => {
                let sense = if kind == ElemKind::ISwitch {
                    self.element_name()
                } else {
                    NameId::NONE
                };
                let control = if two { self.dy() } else { N };
                let initial = self.initial(X::SwitchInitial);
                self.b.detail(Switch {
                    sense,
                    control,
                    initial,
                })
            }
            DetailKind::Line => {
                let row = self.line(kind);
                self.b.detail(row)
            }
        }
    }

    fn initial(&mut self, x: X) -> InitialState {
        if self.maybe(x) {
            if self.g.chance(2) {
                InitialState::Off
            } else {
                InitialState::On
            }
        } else {
            InitialState::Unset
        }
    }

    /// The ic mode when a device ic is drawn (`IcMode::None` when neither mode is in the set).
    fn ic_mode(&mut self) -> IcMode {
        let modes: Vec<IcMode> = [(IcMode::UicOnly, X::IcUicOnly), (IcMode::InOp, X::IcInOp)]
            .into_iter()
            .filter(|&(_, x)| self.has(x))
            .map(|m| m.0)
            .collect();
        if modes.is_empty() || self.g.chance(2) {
            IcMode::None
        } else {
            modes[self.g.below(modes.len() as u32) as usize]
        }
    }

    fn ic_list(&mut self) -> (ExprList, IcMode) {
        let mode = self.ic_mode();
        if mode == IcMode::None {
            return (List::EMPTY, mode);
        }
        let n = 1 + self.g.below(2);
        (self.exprs(n), mode)
    }
}
