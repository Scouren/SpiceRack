//! The run detail rows' fields as data: each row lists its fields once, in IR-text order, with
//! their key and their kind of value. A row in, a walk over its fields out; `DetailFacts`'
//! `handles`, `map_exprs`, `map_names`, `dump` and `parse` are written once over that walk
//! (`table_facts!`) instead of once per row type. `features`, `fault` and `generate` stay with
//! each row: they hold its domain rules.
//!
//! The circuit owner's table (`circuit_fields.rs`) is the same idea for another text: circuit
//! fields are keyed, read in any order, and left out at their default (a choice's first member
//! included). Run text is ordered and positional: a probe, a sweep, a target or an event stands
//! without a key, a choice is written even at its first member (`truncate=box`) or bare
//! (`driven`), a flag is a bare word. One enum for both would give each walker arms the other
//! text never has, so the two tables share the shape, not the enum.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9 (canonical text: fields in declaration order, `NONE` left
//! out); `rewrite/AMEND-A12.md` A23 (`map_exprs` visits what `handles` lists, in its order),
//! A32 (element names).
//!
//! Contents (crate-private): `pub(crate) enum RunField`, `pub(crate) trait RunChoice`,
//! `pub(crate) trait RunFields` (`RunFields::BLANK`, `RunFields::fields`),
//! `pub(crate) fn field_handles`, `pub(crate) fn field_exprs`, `pub(crate) fn field_names`,
//! `pub(crate) fn field_dump`, `pub(crate) fn field_parse`, the defaults `N`, `NN`, `EL`, `NL`,
//! `PROBE`, `SWEEP`; macros (textually scoped: `#[macro_use]` in `lib.rs`) `run_choices!`,
//! `run_fields!`, `table_facts!`

use crate::build::Builder;
use crate::dc_ac::{DcAxis, DcAxisList};
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::Handle;
use crate::id::{ExprId, ExprList, List, NameId, NameList, ScopeId};
use crate::measure::{Event, dump_event, event_handles, parse_event};
use crate::plan::{Target, TargetList, dump_target, parse_target, target_handles};
use crate::probe::{AcPart, Probe, ProbeKind, ProbeList, dump_probe, parse_probe, probe_handles};
use crate::run_exprs::Roots;
use crate::run_names::Names;
use crate::run_words::{
    get, put_expr, put_exprs, put_flag, put_name, put_names, put_word, take_enum, take_exprs,
    take_key, take_name, take_names, take_word, word,
};
use crate::sweep::{
    SweepBounds, SweepEnd, SweepKind, SweepSpec, dump_sweep, parse_sweep, sweep_handles,
};
use crate::text::Words;
use crate::text_expr::Ctx;
use crate::tran::{StepCap, StepCeiling};

/// One field of a run detail row, borrowed mutably, by its text form. A keyed form with an
/// empty key is positional.
#[allow(dead_code)] // ponytail: the forms of the families still by hand; goes with the last one
pub(crate) enum RunField<'a> {
    /// ` key=v`, left out at `NONE`.
    Expr(&'a mut ExprId),
    /// ` key=[v, …]`, left out when empty.
    Exprs(&'a mut ExprList),
    /// ` key=n` (positional ` n`), left out at `NONE`: a net, a definition's or a row's own
    /// name, never an element.
    Name(&'a mut NameId),
    /// As `Name`, an element's name (`map_names` visits it).
    Element(&'a mut NameId),
    /// ` key=(a b)` (positional ` (a b)`), left out when empty: nets.
    Names(&'a mut NameList),
    /// As `Names`, each an element's name (`map_names` visits each).
    Elements(&'a mut NameList),
    /// ` key`, a bare word, when true.
    Word(&'a mut bool),
    /// ` key=word` always (positional ` word`); a missing or unknown word is reported.
    Choice(&'a mut dyn RunChoice),
    /// A positional ` word`, left out at the enum's first member.
    Optional(&'a mut dyn RunChoice),
    /// A positional probe ` (…)`.
    Probe(&'a mut Probe),
    /// A positional sweep ` (…)`.
    Sweep(&'a mut SweepSpec),
    /// A positional target ` (…)`.
    Target(&'a mut Target),
    /// A positional measure event ` (…)`.
    Event(&'a mut Event),
    /// DC axes, each its own fields (`DcAxis`'s table), one after another while a `(` follows.
    DcAxes(&'a mut DcAxisList),
    /// Probes, one after another while a `(` follows.
    Probes(&'a mut ProbeList),
    /// Targets, one after another while a `(` follows.
    Targets(&'a mut TargetList),
    /// A transient's step ceiling, required: ` max_step=v` or ` cap=word`.
    Ceiling(&'a mut StepCeiling),
}

/// A closed enum field seen through its IR-text words (`run_words::word`).
pub(crate) trait RunChoice {
    /// The member's word.
    fn word(&self) -> String;
    /// Whether it is the first member of `ALL`.
    fn first(&self) -> bool;
    /// Sets the member whose word is `w`; false (unchanged) when none is.
    fn set(&mut self, w: &str) -> bool;
}

/// A run detail row whose fields can be walked.
pub(crate) trait RunFields: Copy {
    /// The row with every field at its default (`NONE`, empty, false, the first member).
    const BLANK: Self;
    /// Calls `f` with each field's key and the field, in IR-text order.
    fn fields(&mut self, f: &mut dyn FnMut(&'static str, RunField<'_>));
}

/// Defaults for `run_fields!` rows.
pub(crate) const N: ExprId = ExprId::NONE;
/// An absent name.
pub(crate) const NN: NameId = NameId::NONE;
/// An empty expression list.
pub(crate) const EL: ExprList = List::EMPTY;
/// An empty name list.
#[allow(dead_code)] // as `RunField`
pub(crate) const NL: NameList = List::EMPTY;
/// A probe every reader overwrites.
pub(crate) const PROBE: Probe = Probe {
    kind: ProbeKind::ALL[0],
    part: AcPart::ALL[0],
    terminal: u8::MAX,
    a: List::EMPTY,
    b: List::EMPTY,
    expr: ExprId::NONE,
};
/// A sweep every reader overwrites.
pub(crate) const SWEEP: SweepSpec = SweepSpec {
    kind: SweepKind::ALL[0],
    end: SweepEnd::ALL[0],
    bounds: SweepBounds::ALL[0],
    start: ExprId::NONE,
    stop: ExprId::NONE,
    step: ExprId::NONE,
    points: ExprId::NONE,
    values: List::EMPTY,
};

/// Implements `RunChoice` for enums with `ALL`.
macro_rules! run_choices {
    ($($t:ty),* $(,)?) => {
        $(
            impl $crate::run_fields::RunChoice for $t {
                fn word(&self) -> String {
                    $crate::run_words::word(*self)
                }
                fn first(&self) -> bool {
                    *self == <$t>::ALL[0]
                }
                fn set(&mut self, w: &str) -> bool {
                    match $crate::run_words::member(&<$t>::ALL, w) {
                        Some(e) => {
                            *self = e;
                            true
                        }
                        None => false,
                    }
                }
            }
        )*
    };
}

/// Implements `RunFields`: `Row { field: Form = default, field: Form("key") = default, … }`;
/// the key is the field's name unless given (`""`: positional).
macro_rules! run_fields {
    ($($row:ident { $($f:ident: $form:ident $(($key:literal))? = $default:expr),* $(,)? })*) => {
        $(
            impl $crate::run_fields::RunFields for $row {
                const BLANK: Self = $row { $($f: $default),* };
                fn fields(
                    &mut self,
                    f: &mut dyn FnMut(&'static str, $crate::run_fields::RunField<'_>),
                ) {
                    $( f(run_fields!(@key $f $($key)?),
                         $crate::run_fields::RunField::$form(&mut self.$f)); )*
                }
            }
        )*
    };
    (@key $f:ident $key:literal) => { $key };
    (@key $f:ident) => { stringify!($f) };
}

/// The table-made `DetailFacts` methods (`handles`, `map_exprs`, `map_names`, `dump`, `parse`),
/// inside an `impl DetailFacts for Row` whose row implements `RunFields`.
macro_rules! table_facts {
    () => {
        fn handles(&self, _design: &$crate::design::Design, out: &mut Vec<$crate::facts::Handle>) {
            $crate::run_fields::field_handles(self, _design, out);
        }
        fn map_exprs(&mut self, r: &mut $crate::run_exprs::Roots<'_>) {
            $crate::run_fields::field_exprs(self, r);
        }
        fn map_names(&mut self, r: &mut $crate::run_names::Names<'_>) {
            $crate::run_fields::field_names(self, r);
        }
        fn dump(
            &self,
            design: &$crate::design::Design,
            ctx: &$crate::text_expr::Ctx,
            out: &mut String,
        ) {
            $crate::run_fields::field_dump(self, design, ctx, out);
        }
        fn parse(
            words: &mut $crate::text::Words<'_>,
            scope: $crate::id::ScopeId,
            b: &mut $crate::build::Builder<'_>,
            diags: &mut $crate::diag::Diags,
        ) -> Option<Self> {
            $crate::run_fields::field_parse(words, scope, b, diags)
        }
    };
}

/// Appends every handle of `row`'s fields, in field order (a choice or a word has none).
pub(crate) fn field_handles<R: RunFields>(row: &R, design: &Design, out: &mut Vec<Handle>) {
    let mut row = *row;
    row.fields(&mut |_, field| match field {
        RunField::Expr(e) => out.push(Handle::Expr(*e)),
        RunField::Exprs(l) => out.push(Handle::Exprs(*l)),
        RunField::Name(n) | RunField::Element(n) => out.push(Handle::Name(*n)),
        RunField::Names(l) | RunField::Elements(l) => out.push(Handle::Names(*l)),
        RunField::Word(_) | RunField::Choice(_) | RunField::Optional(_) => {}
        RunField::Probe(p) => probe_handles(p, out),
        RunField::Sweep(s) => sweep_handles(s, out),
        RunField::Target(t) => target_handles(t, out),
        RunField::Event(e) => event_handles(e, out),
        RunField::DcAxes(l) => {
            for axis in get(&design.run.dc_axes, *l).unwrap_or(&[]) {
                field_handles(axis, design, out);
            }
        }
        RunField::Probes(l) => {
            for p in get(&design.run.probes, *l).unwrap_or(&[]) {
                probe_handles(p, out);
            }
        }
        RunField::Targets(l) => {
            for t in get(&design.run.targets, *l).unwrap_or(&[]) {
                target_handles(t, out);
            }
        }
        RunField::Ceiling(c) => {
            if let StepCeiling::Given(e) = c {
                out.push(Handle::Expr(*e));
            }
        }
    });
}

/// Visits `row`'s expression roots as `field_handles` lists them.
pub(crate) fn field_exprs<R: RunFields>(row: &mut R, r: &mut Roots<'_>) {
    row.fields(&mut |_, field| match field {
        RunField::Expr(e) => r.expr(e),
        RunField::Exprs(l) => r.exprs(*l),
        RunField::Probe(p) => r.probe(p),
        RunField::Sweep(s) => r.sweep(s),
        RunField::Event(e) => r.event(e),
        RunField::DcAxes(l) => r.dc_axes(*l),
        RunField::Probes(l) => r.probes(*l),
        RunField::Targets(_) => {}
        RunField::Ceiling(c) => {
            if let StepCeiling::Given(e) = c {
                r.expr(e);
            }
        }
        RunField::Name(_)
        | RunField::Element(_)
        | RunField::Names(_)
        | RunField::Elements(_)
        | RunField::Word(_)
        | RunField::Choice(_)
        | RunField::Optional(_)
        | RunField::Target(_) => {}
    });
}

/// Visits `row`'s element names, in field order.
pub(crate) fn field_names<R: RunFields>(row: &mut R, r: &mut Names<'_>) {
    row.fields(&mut |_, field| match field {
        RunField::Element(n) => r.one(n),
        RunField::Elements(l) => r.each(*l),
        RunField::Probe(p) => r.probe(p),
        RunField::Event(e) => r.event(e),
        RunField::Target(t) => r.target(t),
        RunField::DcAxes(l) => r.dc_axes(*l),
        RunField::Probes(l) => r.probes(*l),
        RunField::Targets(l) => r.targets(*l),
        RunField::Ceiling(_) => {}
        RunField::Expr(_)
        | RunField::Exprs(_)
        | RunField::Name(_)
        | RunField::Names(_)
        | RunField::Word(_)
        | RunField::Choice(_)
        | RunField::Optional(_)
        | RunField::Sweep(_) => {}
    });
}

/// Appends `row`'s fields, each with its leading space.
pub(crate) fn field_dump<R: RunFields>(row: &R, design: &Design, ctx: &Ctx, out: &mut String) {
    let mut row = *row;
    row.fields(&mut |key, field| match field {
        RunField::Expr(e) => put_expr(out, design, ctx, key, *e),
        RunField::Exprs(l) => put_exprs(out, design, ctx, key, *l),
        RunField::Name(n) | RunField::Element(n) => put_name(out, design, key, *n),
        RunField::Names(l) | RunField::Elements(l) => put_names(out, design, key, *l),
        RunField::Word(on) => put_flag(out, *on, key),
        RunField::Choice(c) if key.is_empty() => put_word(out, &c.word()),
        RunField::Choice(c) => put_word(out, &format!("{key}={}", c.word())),
        RunField::Optional(c) => {
            if !c.first() {
                put_word(out, &c.word());
            }
        }
        RunField::Probe(p) => dump_probe(design, ctx, p, out),
        RunField::Sweep(s) => dump_sweep(design, ctx, s, out),
        RunField::Target(t) => dump_target(design, ctx, t, out),
        RunField::Event(e) => dump_event(design, ctx, e, out),
        RunField::DcAxes(l) => {
            for axis in design.dc_axes(*l) {
                field_dump(axis, design, ctx, out);
            }
        }
        RunField::Probes(l) => {
            for p in design.probes(*l) {
                dump_probe(design, ctx, p, out);
            }
        }
        RunField::Targets(l) => {
            for t in design.targets(*l) {
                dump_target(design, ctx, t, out);
            }
        }
        RunField::Ceiling(StepCeiling::Given(e)) => put_expr(out, design, ctx, "max_step", *e),
        RunField::Ceiling(StepCeiling::Rule(cap)) => {
            put_word(out, &format!("cap={}", word(*cap)));
        }
    });
}

/// Reads a row's fields in order from `R::BLANK`; `None` after the first report.
pub(crate) fn field_parse<R: RunFields>(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<R> {
    let mut row = R::BLANK;
    let mut ok = true;
    row.fields(&mut |key, field| {
        if ok {
            ok = read(words, scope, b, diags, key, field);
        }
    });
    ok.then_some(row)
}

// ---- private ----

/// Reads one field; false after a report.
fn read(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
    key: &str,
    field: RunField<'_>,
) -> bool {
    match field {
        RunField::Expr(e) => {
            if take_key(words, key) {
                *e = words.value(scope, b, diags);
            }
            true
        }
        RunField::Exprs(l) => {
            *l = take_exprs(words, key, scope, b, diags);
            true
        }
        RunField::Name(n) | RunField::Element(n) if key.is_empty() => {
            *n = words.name(b, diags);
            true
        }
        RunField::Name(n) | RunField::Element(n) => {
            *n = take_name(words, key, b, diags);
            true
        }
        RunField::Names(l) | RunField::Elements(l) => {
            *l = take_names(words, key, b, diags);
            true
        }
        RunField::Word(on) => {
            *on = take_word(words, key);
            true
        }
        RunField::Choice(c) => {
            let keyed = key.is_empty() || take_key(words, key);
            let w = words.peek().map(str::to_owned);
            let set = keyed && w.is_some_and(|w| c.set(&w));
            if set {
                words.next();
            } else {
                words.error(diags);
            }
            set
        }
        RunField::Optional(c) => {
            if let Some(w) = words.peek().map(str::to_owned)
                && c.set(&w)
            {
                words.next();
            }
            true
        }
        RunField::Probe(p) => parse_probe(words, scope, b, diags)
            .map(|x| *p = x)
            .is_some(),
        RunField::Sweep(s) => parse_sweep(words, scope, b, diags)
            .map(|x| *s = x)
            .is_some(),
        RunField::Target(t) => parse_target(words, b, diags).map(|x| *t = x).is_some(),
        RunField::Event(e) => parse_event(words, scope, b, diags)
            .map(|x| *e = x)
            .is_some(),
        RunField::DcAxes(l) => {
            let mut axes = Vec::new();
            // each round consumes an axis or returns
            while words.peek() == Some("(") {
                let Some(axis) = field_parse::<DcAxis>(words, scope, b, diags) else {
                    return false;
                };
                axes.push(axis);
            }
            *l = b.dc_axes(&axes);
            true
        }
        RunField::Probes(l) => {
            let mut probes = Vec::new();
            // each round consumes a probe or returns
            while words.peek() == Some("(") {
                let Some(p) = parse_probe(words, scope, b, diags) else {
                    return false;
                };
                probes.push(p);
            }
            *l = b.probes(&probes);
            true
        }
        RunField::Targets(l) => {
            let mut targets = Vec::new();
            // each round consumes a target or returns
            while words.peek() == Some("(") {
                let Some(t) = parse_target(words, b, diags) else {
                    return false;
                };
                targets.push(t);
            }
            *l = b.targets(&targets);
            true
        }
        RunField::Ceiling(c) => {
            let given = if take_key(words, "max_step") {
                words.value(scope, b, diags)
            } else {
                ExprId::NONE
            };
            if !given.is_none() {
                *c = StepCeiling::Given(given);
                true
            } else if take_key(words, "cap") {
                match take_enum(words, &StepCap::ALL, diags) {
                    Some(cap) => {
                        *c = StepCeiling::Rule(cap);
                        true
                    }
                    None => false,
                }
            } else {
                words.error(diags);
                false
            }
        }
    }
}
