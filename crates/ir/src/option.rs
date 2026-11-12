//! Simulator state as items: options, the environment (temperature, nominal temperature), and
//! initial conditions and node sets. A reader's `.options`/`.temp`/`.ic`/`.nodeset` (and their
//! Spectre, VACASK and Xyce forms) in, rows out.
//!
//! The environment is always explicit: a reader fills its simulator's defaults (HSPICE 25 °C for
//! both, the others 27 °C), so a deck read from HSPICE and written for ngspice keeps 25 °C
//! (`INV-semantics.md` 4.01). Several `.temp` lines: the reader keeps its simulator's winner;
//! a list of temperatures is a `Loop` over `Target::TEMP` (ngspice runs one temperature only:
//! its reader keeps 27 °C and notes the list, 4.02).
//!
//! Where an initial condition applies is a field, because simulators disagree
//! (`INV-semantics.md` 8.21): ngspice forces a `.ic` node in the transient's operating point
//! only, Xyce also in `.op` and every `.dc` point. A `NodeSet` is legal in a subcircuit body
//! (LTspice, Xyce: `INV-constructs.md` D88, I25): it then applies to every instance, and its
//! `node` path is relative to that body. At the top its path may reach into an instance (9.01).
//! A device's own initial state (VACASK has no capacitor `ic=` parameter: the value is an entry
//! of the transient's `ic` list; Xyce `.INITCOND`) is a `NodeSetKind::Device` row.
//!
//! Cites: `rewrite/IR-RUN.md` §3; `rewrite/INV-constructs.md` I1, I7–I11, I15, I20, I25, G63;
//! D6, D88; `rewrite/INV-semantics.md` 4.01–4.03, 8.21, 9.01.
//!
//! IR text:
//!
//! ```text
//! option <key> [value=v]                       option private <name> <dialect> [value=v]
//! env temp=v tnom=v
//! nodeset ic|nodeset|device transient|every_op [next_only] [node=(…)] [reference=(…)] value=v
//! ```
//!
//! Contents: `pub struct OptionRow`, `pub struct Env`, `pub struct NodeSet`,
//! `pub enum NodeSetKind`, `pub enum IcScope`; on each enum: `ALL`, `COUNT`; `RowFacts` for
//! `OptionRow`, `Env` and `NodeSet`

use crate::analysis_facts::report;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{Handle, RowFacts};
use crate::feature::{Feature, FeatureSet};
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::{Gen, Reads};
use crate::id::{ExprId, ItemRef, NameId, NameList, Ref, ScopeId};
use crate::item::{DefKind, ScopeKind};
use crate::option_key::OptionKey;
use crate::run_extra::RunExtra;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::after_run_item;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    flag, in_scope_kind, put_expr, put_flag, put_name, put_names, put_word, take_enum, take_expr,
    take_names, take_word, trim_lead, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::Ctx;

/// One simulator option. Applies to every run item after it (an option set between analyses
/// is a state change: VACASK `options` in `control`, ngspice `option` in a script); a reader of
/// a dialect whose options are deck-global puts its options before the first run item.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptionRow {
    /// The portable key, or `OptionKey::Private`.
    pub key: OptionKey,
    /// `Private`: the option's name as its dialect spells it (Xyce: `package.key`, e.g.
    /// `timeint.reltol`). `NONE` for every other key.
    pub private: NameId,
    /// `Private`: the dialect whose option it is (`ngspice`, `xyce`, …; ngspice's compatibility
    /// mode is `ngbehavior` of `ngspice`, read by the backend from here). `NONE` otherwise.
    pub dialect: NameId,
    /// The value in the key's unit; `NONE` for a flag. A string value is an `Expr::Str`.
    pub value: ExprId,
}

/// The circuit temperature and the nominal temperature, °C. Both always present (IR-RUN §3).
/// One per deck, before its first run item; a change between runs is an `Alter` of
/// `Target::TEMP`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {
    /// The circuit temperature; never `NONE`. May be an expression of parameters (ngspice
    /// `.temp {t}`).
    pub temp: ExprId,
    /// The temperature model parameters were measured at; never `NONE`.
    pub tnom: ExprId,
}

/// An initial condition or a node set on one node, or on the voltage between two.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSet {
    /// A forced initial condition or a starting guess.
    pub kind: NodeSetKind,
    /// `Ic`: which operating points hold it. `Nodeset`: always `EveryOp` (checked; a guess
    /// serves every solve).
    pub applies: IcScope,
    /// Whether it applies to the next analysis only (VACASK's analysis-line `ic=[…]` and
    /// `nodeset=[…]`), instead of to every analysis after it.
    pub next_only: bool,
    /// The node: the instance path then the net, relative to the item's scope (a subcircuit
    /// body: each instance's own nodes). `Device`: the element's path. Empty only for
    /// `Nodeset` with every node (ngspice `.nodeset all=v`).
    pub node: NameList,
    /// The second node of a voltage across two nodes (VACASK `ic=["1";"2";1.0]`), or empty for
    /// a node voltage to ground; always empty for `Device`.
    pub reference: NameList,
    /// The voltage, V (`Device`: the state in its own unit: a capacitor's voltage, an
    /// inductor's current); never `NONE`. May be an expression of parameters (Xyce
    /// `.ic v(1)={V1}`).
    pub value: ExprId,
}

/// What a [`NodeSet`] does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum NodeSetKind {
    /// The node is held at the value while the operating point is solved, then released
    /// (`.ic`, Spectre `ic`, VACASK `ic=`); under UIC it is the transient's starting value.
    Ic,
    /// The value is a starting guess for the operating point (`.nodeset`, `nodeset=`).
    Nodeset,
    /// The element's own state starts at the value (VACASK's transient `ic` entry for a
    /// device, Xyce `.INITCOND`); held per `applies` like `Ic`. An instance's `IC=` parameter is
    /// the circuit owner's field; a reader whose simulator states it here uses this.
    Device,
}

/// Which operating points a `.ic` holds its node in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum IcScope {
    /// Only the transient's initial operating point (ngspice: not `.op`, `.dc` or `.ac`).
    Transient,
    /// Every operating point: `.op`, each `.dc` point, the transient's (Xyce).
    EveryOp,
}

const _: () = assert!(size_of::<OptionRow>() == 16);
const _: () = assert!(size_of::<Env>() == 8);
const _: () = assert!(size_of::<NodeSet>() == 24);

impl NodeSetKind {
    /// Every kind, in declaration order.
    pub const ALL: [NodeSetKind; 3] = [NodeSetKind::Ic, NodeSetKind::Nodeset, NodeSetKind::Device];
    /// Kind count.
    pub const COUNT: usize = Self::ALL.len();
}

impl IcScope {
    /// Every scope, in declaration order.
    pub const ALL: [IcScope; 2] = [IcScope::Transient, IcScope::EveryOp];
    /// Scope count.
    pub const COUNT: usize = Self::ALL.len();
}

impl RowFacts for OptionRow {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Name(self.private),
            Handle::Name(self.dialect),
            Handle::Expr(self.value),
        ]);
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let _ = (design, at, f); // no reference
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        *set = set.with(Feature::Option(self.key));
        // `OptionMidRun` is a fact of the whole design: `analysis::design_features`.
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let named = !self.private.is_none() && !self.dialect.is_none();
        let ok = if self.key == OptionKey::Private {
            named
        } else {
            self.private.is_none()
                && self.dialect.is_none()
                && self.value.is_none() == self.key.is_flag()
        };
        if !ok {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        out.push_str(&word(self.key));
        put_name(out, design, "", self.private);
        put_name(out, design, "", self.dialect);
        put_expr(out, design, ctx, "value", self.value);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let Some(key) = take_enum(words, &OptionKey::ALL, diags) else {
            return ItemRef::NONE;
        };
        let (mut private, mut dialect) = (NameId::NONE, NameId::NONE);
        if key == OptionKey::Private {
            private = words.name(b, diags);
            dialect = words.name(b, diags);
        }
        let value = take_expr(words, "value", scope, b, diags);
        b.push(
            scope,
            OptionRow {
                key,
                private,
                dialect,
                value,
            },
            words.span(),
        )
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let design = b.design();
        let pos = design.items(scope).len();
        if !has(features, RunExtra::OptionMidRun) && after_run_item(design, scope, pos) {
            return ItemRef::NONE;
        }
        let keys: Vec<OptionKey> = OptionKey::ALL
            .into_iter()
            .filter(|&k| features.contains(Feature::Option(k)))
            .collect();
        if keys.is_empty() {
            return ItemRef::NONE;
        }
        let key = keys[g.below(keys.len() as u32) as usize];
        let row = if key == OptionKey::Private {
            let (private, dialect) = (g.fresh_name(b), g.fresh_name(b));
            let value = if g.chance(2) {
                g.expr(b, features, Reads::Constant)
            } else {
                ExprId::NONE
            };
            OptionRow {
                key,
                private,
                dialect,
                value,
            }
        } else {
            let value = if key.is_flag() {
                ExprId::NONE
            } else {
                g.expr(b, features, Reads::Constant)
            };
            OptionRow {
                key,
                private: NameId::NONE,
                dialect: NameId::NONE,
                value,
            }
        };
        b.push(scope, row, Span::NONE)
    }
}

impl RowFacts for Env {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([Handle::Expr(self.temp), Handle::Expr(self.tnom)]);
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let _ = (design, at, f); // no reference
    }

    fn features(&self, _design: &Design, _at: ItemRef, _scope: ScopeId, _set: &mut FeatureSet) {}

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if self.temp.is_none() || self.tnom.is_none() {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let at = out.len();
        put_expr(out, design, ctx, "temp", self.temp);
        put_expr(out, design, ctx, "tnom", self.tnom);
        trim_lead(out, at);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let temp = take_expr(words, "temp", scope, b, diags);
        let tnom = take_expr(words, "tnom", scope, b, diags);
        b.push(scope, Env { temp, tnom }, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let temp = g.expr(b, features, Reads::Constant);
        let tnom = g.expr(b, features, Reads::Constant);
        b.push(scope, Env { temp, tnom }, Span::NONE)
    }
}

impl RowFacts for NodeSet {
    fn handles(&self, _design: &Design, out: &mut Vec<Handle>) {
        out.extend([
            Handle::Names(self.node),
            Handle::Names(self.reference),
            Handle::Expr(self.value),
        ]);
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let _ = (design, at, f); // no reference
    }

    fn features(&self, design: &Design, _at: ItemRef, scope: ScopeId, set: &mut FeatureSet) {
        let held = self.kind != NodeSetKind::Nodeset;
        flag(
            set,
            held && self.applies == IcScope::EveryOp,
            RunExtra::IcEveryOp,
        );
        flag(set, self.next_only, RunExtra::IcNextOnly);
        flag(set, self.reference.len > 0, RunExtra::IcDifferential);
        flag(
            set,
            self.kind == NodeSetKind::Nodeset && self.node.len == 0,
            RunExtra::NodesetAll,
        );
        flag(
            set,
            self.node.len > 1 || self.reference.len > 1,
            RunExtra::NodeSetHierarchical,
        );
        flag(set, self.kind == NodeSetKind::Device, RunExtra::IcDevice);
        let subckt = in_scope_kind(design, scope, ScopeKind::Subckt);
        flag(set, subckt, RunExtra::NodeSetInSubckt);
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        let ok = !self.value.is_none()
            && (self.kind != NodeSetKind::Nodeset || self.applies == IcScope::EveryOp)
            && (self.node.len > 0 || self.kind == NodeSetKind::Nodeset)
            && (self.kind != NodeSetKind::Device || self.reference.len == 0);
        if !ok {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        out.push_str(&word(self.kind));
        put_word(out, &word(self.applies));
        put_flag(out, self.next_only, "next_only");
        put_names(out, design, "node", self.node);
        put_names(out, design, "reference", self.reference);
        put_expr(out, design, ctx, "value", self.value);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let Some(kind) = take_enum(words, &NodeSetKind::ALL, diags) else {
            return ItemRef::NONE;
        };
        let Some(applies) = take_enum(words, &IcScope::ALL, diags) else {
            return ItemRef::NONE;
        };
        let next_only = take_word(words, "next_only");
        let node = take_names(words, "node", b, diags);
        let reference = take_names(words, "reference", b, diags);
        let value = take_expr(words, "value", scope, b, diags);
        let row = NodeSet {
            kind,
            applies,
            next_only,
            node,
            reference,
            value,
        };
        b.push(scope, row, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        if !has(features, RunExtra::NodeSetInSubckt)
            && in_scope_kind(b.design(), scope, ScopeKind::Subckt)
        {
            return ItemRef::NONE;
        }
        let mut kinds = vec![NodeSetKind::Ic, NodeSetKind::Nodeset];
        if has(features, RunExtra::IcDevice) {
            kinds.push(NodeSetKind::Device);
        }
        let kind = kinds[g.below(kinds.len() as u32) as usize];
        let applies = if kind == NodeSetKind::Nodeset
            || (has(features, RunExtra::IcEveryOp) && g.chance(2))
        {
            IcScope::EveryOp
        } else {
            IcScope::Transient
        };
        let next_only = has(features, RunExtra::IcNextOnly) && g.chance(3);
        let all =
            kind == NodeSetKind::Nodeset && has(features, RunExtra::NodesetAll) && g.chance(3);
        let node = if all {
            crate::id::List::EMPTY
        } else {
            let deep = has(features, RunExtra::NodeSetHierarchical) && g.chance(3);
            let names: Vec<NameId> = (0..1 + deep as usize).map(|_| g.fresh_name(b)).collect();
            b.name_ids(&names)
        };
        let differential =
            kind != NodeSetKind::Device && has(features, RunExtra::IcDifferential) && g.chance(3);
        let reference = if differential {
            let r = g.fresh_name(b);
            b.name_ids(&[r])
        } else {
            crate::id::List::EMPTY
        };
        let value = g.expr(b, features, Reads::Constant);
        let row = NodeSet {
            kind,
            applies,
            next_only,
            node,
            reference,
            value,
        };
        b.push(scope, row, Span::NONE)
    }
}
