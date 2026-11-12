//! Probes: one typed output quantity (a node voltage, a branch or terminal current, a device
//! quantity, a power, an expression, or a category of them), read by `Save`, `Print`, `Measure`
//! and the analyses that name an output. A reader's output spelling in (`v(x1.n)`, `V(X1:n)`,
//! `i(v1)`, `v1#branch`, `IS(M1)`, `I3(M1)`, `M1:s`, `@m1[gm]`, `vdb(out)`), one row out.
//!
//! Paths are name lists, the instance names then the net or element: no separator character and
//! no case rule lives here (`INV-semantics.md` 9.01). A current is positive INTO the device at
//! the terminal it names: `I` is the current entering the element's first terminal (and leaving
//! its second), `ITerminal` the current entering that terminal from the net. A writer whose
//! simulator reports the other sign negates or inserts a 0 V probe (9.02).
//!
//! There is no `All` kind: each simulator's "all" means another set (ngspice: node voltages and
//! voltage-source currents; LTspice: also device currents, but no subcircuit-internal node), so
//! each reader expands its own `all` and its own default save set into the categories it means
//! (9.03–9.05).
//!
//! Cites: `rewrite/IR-RUN.md` §4; `rewrite/INV-constructs.md` H1, H3, H12, H13, H23, H25–H27;
//! `rewrite/INV-semantics.md` 9.01–9.06; `DESIGN-probe-terminals.md` §2–§3.
//!
//! Contents: `pub struct Probe`, `pub enum ProbeKind`, `pub enum AcPart`, `pub type ProbeList`;
//! on each enum: `ALL`, `COUNT`; `Probe::category(ProbeKind) -> Probe`,
//! `Probe::with_part(self, AcPart) -> Probe`, `pub(crate) ProbeKind::is_category`,
//! `Design::probes(&self, ProbeList) -> &[Probe]`
//! - the run rows' probe facts: `pub(crate) fn probe_handles`, `pub(crate) fn probe_valid`,
//!   `pub(crate) fn probe_features`, `pub(crate) fn dump_probe`, `pub(crate) fn parse_probe`,
//!   `pub(crate) fn gen_probe`
//!
//! IR text: `(kind [part] [terminal=k] [(a …)] [(b …)] [expr=v])`, e.g. `(v (x1 out))`,
//! `(vdiff (a) (b))`, `(i mag (v1))`, `(i_terminal terminal=2 (m1))`, `(param (m1) (gm))`,
//! `(expr expr=(v(a) * 2))`, `(all_v)`.

use crate::build::Builder;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, List, NameList, ScopeId};
use crate::run_extra::RunExtra;
use crate::run_words::{
    close, get, member, open, put_expr, put_names, put_word, take_enum, take_expr, take_key,
    take_names, word,
};
use crate::text::Words;
use crate::text_expr::Ctx;

/// One output quantity. `a`, `b`, `terminal` and `expr` are read per [`ProbeKind`]; every field
/// a kind does not read is empty, `u8::MAX` or `NONE` (checked).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Probe {
    /// What is probed.
    pub kind: ProbeKind,
    /// Which part of a complex (small-signal) value; `None` for the value itself.
    pub part: AcPart,
    /// `ITerminal`: the terminal's role index in the element kind's terminal order (the
    /// circuit owner's per-`ElemKind` doc in `element.rs`; HSPICE `I3(M1)` is index 2, ngspice `i(m1:s)` the index of `s`).
    /// `u8::MAX` otherwise.
    pub terminal: u8,
    /// `V`, `Vdiff`: the node path. `I`, `ITerminal`, `Param`, `Power`: the element path.
    /// Empty for the other kinds.
    pub a: NameList,
    /// `Vdiff`: the second node path (the voltage is `a − b`). `Param`: one name, the device
    /// quantity as its source spelled it (`gm`, `id`). Empty otherwise.
    pub b: NameList,
    /// `Expr`: the expression (it may read `V`, `I`, `Time`, `Freq` and measures). `NONE`
    /// otherwise.
    pub expr: ExprId,
}

/// The kind of a [`Probe`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum ProbeKind {
    /// A node's voltage to ground.
    V,
    /// The voltage between two nodes, `a − b` (Spectre has none: its writer saves both).
    Vdiff,
    /// The element's branch current, positive into its first terminal.
    I,
    /// The current into one terminal of the element (`.probe i(m1:s)`, Xyce `IS(M1)`, Spectre
    /// `M1:s`).
    ITerminal,
    /// A device's operating-point quantity (ngspice `@m1[gm]`, Xyce `N(m1:x)`, VACASK
    /// `p(m1,i_d)`, Spectre `m1:gm`). The reader canonicalises the name where a canonical
    /// quantity table exists (gap G-OPQUANT, `INV-semantics.md` 9.06); until then it is the
    /// name its simulator spells.
    Param,
    /// The power the element absorbs, W (`p(dev)`, Spectre `dev:pwr`).
    Power,
    /// The value of an expression (ngspice `par('…')`, Xyce `{…}`).
    Expr,
    /// Every node voltage, at every depth of the hierarchy.
    AllV,
    /// Every node voltage of the top level only (LTspice's default save set).
    AllVTop,
    /// Every voltage source's branch current.
    AllSourceI,
    /// Every device terminal current (ngspice `.probe alli`, `savecurrents`; Xyce `I(*)`;
    /// Spectre `:currents`).
    AllI,
    /// Every subcircuit pin current (LTspice `Ix(*)`).
    AllPinI,
    /// The output noise of the noise analysis (`onoise`), V/√Hz.
    OutputNoise,
    /// The input-referred noise of the noise analysis (`inoise`).
    InputNoise,
}

/// Which part of a complex value a probe reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum AcPart {
    /// The value itself (complex in a small-signal analysis).
    None,
    /// The real part (`vr`).
    Re,
    /// The imaginary part (`vi`).
    Im,
    /// The magnitude (`vm`).
    Mag,
    /// The phase in degrees (HSPICE, Xyce `vp`).
    PhaseDeg,
    /// The phase in radians (ngspice `vp`).
    PhaseRad,
    /// The magnitude in decibels, `20·log10(|x|)` (`vdb`).
    Db,
}

/// An ordered list of probes, a range into the run owner's probe pool.
pub type ProbeList = List<Probe>;

const _: () = assert!(size_of::<Probe>() == 24);

impl Probe {
    /// A category probe (`AllV`, `AllVTop`, `AllSourceI`, `AllI`, `AllPinI`, `OutputNoise`,
    /// `InputNoise`): no path, no expression. Asserts `kind` is one of those.
    pub const fn category(kind: ProbeKind) -> Probe {
        assert!(kind.is_category(), "Probe::category takes a category kind");
        Probe {
            kind,
            part: AcPart::None,
            terminal: u8::MAX,
            a: List::EMPTY,
            b: List::EMPTY,
            expr: ExprId::NONE,
        }
    }

    /// The same probe reading another part.
    pub const fn with_part(self, part: AcPart) -> Probe {
        Probe { part, ..self }
    }
}

impl ProbeKind {
    /// Every kind, in declaration order.
    pub const ALL: [ProbeKind; 14] = [
        ProbeKind::V,
        ProbeKind::Vdiff,
        ProbeKind::I,
        ProbeKind::ITerminal,
        ProbeKind::Param,
        ProbeKind::Power,
        ProbeKind::Expr,
        ProbeKind::AllV,
        ProbeKind::AllVTop,
        ProbeKind::AllSourceI,
        ProbeKind::AllI,
        ProbeKind::AllPinI,
        ProbeKind::OutputNoise,
        ProbeKind::InputNoise,
    ];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();

    /// Whether the kind is a category (no path, no expression).
    pub(crate) const fn is_category(self) -> bool {
        match self {
            ProbeKind::V
            | ProbeKind::Vdiff
            | ProbeKind::I
            | ProbeKind::ITerminal
            | ProbeKind::Param
            | ProbeKind::Power
            | ProbeKind::Expr => false,
            ProbeKind::AllV
            | ProbeKind::AllVTop
            | ProbeKind::AllSourceI
            | ProbeKind::AllI
            | ProbeKind::AllPinI
            | ProbeKind::OutputNoise
            | ProbeKind::InputNoise => true,
        }
    }
}

impl AcPart {
    /// Every part, in declaration order.
    pub const ALL: [AcPart; 7] = [
        AcPart::None,
        AcPart::Re,
        AcPart::Im,
        AcPart::Mag,
        AcPart::PhaseDeg,
        AcPart::PhaseRad,
        AcPart::Db,
    ];
    /// Part count.
    pub const COUNT: usize = Self::ALL.len();
}

impl Design {
    /// The probes of a list. Asserts the range is inside the probe pool.
    pub fn probes(&self, list: ProbeList) -> &[Probe] {
        get(&self.run.probes, list).expect("a probe list inside the probe pool")
    }
}

/// Appends a probe's handles in field order; its expression may read the circuit.
pub(crate) fn probe_handles(p: &Probe, out: &mut Vec<Handle>) {
    out.extend([
        Handle::Names(p.a),
        Handle::Names(p.b),
        Handle::ExprDyn(p.expr),
    ]);
}

/// Whether the probe holds exactly the fields its kind reads (the `Probe` doc).
pub(crate) fn probe_valid(p: &Probe) -> bool {
    let (a, b) = (p.a.len > 0, p.b.len > 0);
    let expr = !p.expr.is_none();
    let terminal = p.terminal != u8::MAX;
    let fields = match p.kind {
        ProbeKind::V | ProbeKind::I | ProbeKind::Power => (true, false, false, false),
        ProbeKind::Vdiff => (true, true, false, false),
        ProbeKind::ITerminal => (true, false, true, false),
        ProbeKind::Param => (true, p.b.len == 1, false, false),
        ProbeKind::Expr => (false, false, false, true),
        ProbeKind::AllV
        | ProbeKind::AllVTop
        | ProbeKind::AllSourceI
        | ProbeKind::AllI
        | ProbeKind::AllPinI
        | ProbeKind::OutputNoise
        | ProbeKind::InputNoise => (false, false, false, false),
    };
    let b_ok = if p.kind == ProbeKind::Param {
        fields.1
    } else {
        b == fields.1
    };
    a == fields.0 && b_ok && terminal == fields.2 && expr == fields.3
}

/// Adds the probe's `RunExtra` features.
pub(crate) fn probe_features(p: &Probe, set: &mut FeatureSet) {
    let mut add = |x: RunExtra| *set = set.with(Feature::Run(x));
    let kind = match p.kind {
        ProbeKind::V => None,
        ProbeKind::Vdiff => Some(RunExtra::ProbeVdiff),
        ProbeKind::I => Some(RunExtra::ProbeI),
        ProbeKind::ITerminal => Some(RunExtra::ProbeITerminal),
        ProbeKind::Param => Some(RunExtra::ProbeParam),
        ProbeKind::Power => Some(RunExtra::ProbePower),
        ProbeKind::Expr => Some(RunExtra::ProbeExpr),
        ProbeKind::AllV => Some(RunExtra::ProbeAllV),
        ProbeKind::AllVTop => Some(RunExtra::ProbeAllVTop),
        ProbeKind::AllSourceI => Some(RunExtra::ProbeAllSourceI),
        ProbeKind::AllI => Some(RunExtra::ProbeAllI),
        ProbeKind::AllPinI => Some(RunExtra::ProbeAllPinI),
        ProbeKind::OutputNoise | ProbeKind::InputNoise => Some(RunExtra::ProbeNoise),
    };
    if let Some(x) = kind {
        add(x);
    }
    let part = match p.part {
        AcPart::None => None,
        AcPart::Re => Some(RunExtra::PartRe),
        AcPart::Im => Some(RunExtra::PartIm),
        AcPart::Mag => Some(RunExtra::PartMag),
        AcPart::PhaseDeg => Some(RunExtra::PartPhaseDeg),
        AcPart::PhaseRad => Some(RunExtra::PartPhaseRad),
        AcPart::Db => Some(RunExtra::PartDb),
    };
    if let Some(x) = part {
        add(x);
    }
    if p.a.len > 1 || (p.kind == ProbeKind::Vdiff && p.b.len > 1) {
        add(RunExtra::ProbeHierarchical);
    }
}

/// Appends ` (kind …)`.
pub(crate) fn dump_probe(design: &Design, ctx: &Ctx, p: &Probe, out: &mut String) {
    out.push_str(" (");
    out.push_str(&word(p.kind));
    if p.part != AcPart::None {
        put_word(out, &word(p.part));
    }
    if p.terminal != u8::MAX {
        put_word(out, &format!("terminal={}", p.terminal));
    }
    put_names(out, design, "", p.a);
    put_names(out, design, "", p.b);
    put_expr(out, design, ctx, "expr", p.expr);
    out.push(')');
}

/// Reads `(kind …)`; `None` after a report when it is not one.
pub(crate) fn parse_probe(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<Probe> {
    if !open(words, diags) {
        return None;
    }
    let kind = take_enum(words, &ProbeKind::ALL, diags)?;
    let part = match words.peek().and_then(|w| member(&AcPart::ALL[1..], w)) {
        Some(part) => {
            words.next();
            part
        }
        None => AcPart::None,
    };
    let mut terminal = u8::MAX;
    if take_key(words, "terminal") {
        match words.next().and_then(|w| w.parse::<u8>().ok()) {
            Some(t) if t != u8::MAX => terminal = t,
            _ => {
                words.error(diags);
                return None;
            }
        }
    }
    let a = take_names(words, "", b, diags);
    let names_b = take_names(words, "", b, diags);
    let expr = take_expr(words, "expr", scope, b, diags);
    let p = Probe {
        kind,
        part,
        terminal,
        a,
        b: names_b,
        expr,
    };
    close(words, diags).then_some(p)
}

/// A random valid probe using only `features` (a node voltage needs none; `paths` limits it
/// to the kinds with a node or element path, for analysis outputs).
#[cfg(any(test, feature = "gen"))]
pub(crate) fn gen_probe(
    g: &mut Gen,
    b: &mut Builder<'_>,
    features: &FeatureSet,
    paths: bool,
) -> Probe {
    let mut kinds = vec![ProbeKind::V];
    for k in ProbeKind::ALL.into_iter().skip(1) {
        let mut one = FeatureSet::EMPTY;
        probe_features(
            &Probe {
                kind: k,
                ..Probe::category(ProbeKind::AllV)
            },
            &mut one,
        );
        let allowed = one.iter().all(|f| features.contains(f));
        if allowed && (!paths || matches!(k, ProbeKind::Vdiff | ProbeKind::I)) {
            kinds.push(k);
        }
    }
    let kind = kinds[g.below(kinds.len() as u32) as usize];
    let path = |g: &mut Gen, b: &mut Builder<'_>| {
        let deep = features.contains(Feature::Run(RunExtra::ProbeHierarchical)) && g.chance(3);
        let names: Vec<_> = (0..1 + deep as usize).map(|_| g.fresh_name(b)).collect();
        b.name_ids(&names)
    };
    let mut p = Probe::category(ProbeKind::AllV);
    p.kind = kind;
    match kind {
        ProbeKind::V | ProbeKind::I | ProbeKind::Power => p.a = path(g, b),
        ProbeKind::Vdiff => {
            p.a = path(g, b);
            p.b = path(g, b);
        }
        ProbeKind::ITerminal => {
            p.a = path(g, b);
            p.terminal = g.below(4) as u8;
        }
        ProbeKind::Param => {
            p.a = path(g, b);
            let q = g.fresh_name(b);
            p.b = b.name_ids(&[q]);
        }
        ProbeKind::Expr => p.expr = g.expr(b, features, Reads::Circuit),
        ProbeKind::AllV
        | ProbeKind::AllVTop
        | ProbeKind::AllSourceI
        | ProbeKind::AllI
        | ProbeKind::AllPinI
        | ProbeKind::OutputNoise
        | ProbeKind::InputNoise => {}
    }
    let parts: Vec<AcPart> = AcPart::ALL
        .into_iter()
        .filter(|&part| {
            let mut one = FeatureSet::EMPTY;
            probe_features(
                &Probe {
                    part,
                    ..Probe::category(ProbeKind::AllV)
                },
                &mut one,
            );
            one.iter()
                .all(|f| features.contains(f) || f == Feature::Run(RunExtra::ProbeAllV))
        })
        .collect();
    p.part = parts[g.below(parts.len() as u32) as usize];
    debug_assert!(probe_valid(&p));
    p
}
