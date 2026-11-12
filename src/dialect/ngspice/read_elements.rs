//! Element cards: every `Tag::Instance` whose letter is not `V` or `I` (R C L K, D Q M J Z,
//! S W, T O U Y P, E F G H B, X N A) → one `Element` row with its detail row, or one `Opaque`
//! with a coded note. The card's letter and its shape pick the `ElemKind`; the IR name is the
//! whole folded name, letter included (R23); nodes fold through `Folder::node` (`gnd` is `0`).
//!
//! The card text after the name is split here by ngspice's own token rules (`INPgetNetTok`,
//! `gettok`: blanks and commas separate, `k=v` is one parameter whatever the blanks around `=`,
//! `{…}`, `'…'` and `(…)` are one item, a word keeps the parentheses it touches: `poly(2)`), and
//! each letter's rules are its `inp2<letter>.c` and the `inpcom.c` rewrite before it, in the
//! child modules. A value is a card number (`INPevaluate`, `number::card`) or a braced
//! expression read by the evaluator its class picks (R14); a B-source field is always the
//! B-source parser's (`inp_bsource_compat`). Every expression parses before the first push, so a
//! refusal leaves no row. A master is pushed unresolved and queued for the bind step (R3).
//!
//! Cites: ngspice-45 `spicelib/parser/inp2*.c`, `frontend/inpcom.c`, `inpgtok.c`;
//! `rewrite/DIALECT-NGSPICE.md` §3; `rewrite/S3-AREAS.md`; `rewrite/RULINGS.md` R1, R3, R14,
//! R23.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! the codes as constants (`pub(super) const MALFORMED`, `UNREAD_FORM`, `DROPPED`, `TC_BASE`,
//! `FIRST_BRACE`, `PRIVATE_FORM`, `DUPLICATE_KEY`, `PORT_COUNT`, `MODEL_ORDER`), `pub(super) struct Item`, `pub(super) enum Kind`,
//! `pub(super) enum Val`, `pub(super) struct Card`, `Card::param(&mut self, &str, Val)`, `Item::as_value(&self) -> String`,
//! `pub(super) fn element(lw: &mut Lower<'_, '_>, st: Stmt)`, `pub(super) fn val`, `b_tree`,
//! `emit`, `head`, `push_done`, `number_word`, `literal`,
//! `pub(super) fn instance_temp(&ir::Design, &ir::Element) -> (bool, bool)`,
//! `pub(super) fn bound(&ir::Design, &mut ir::Diags)`, `pub(super) const TEMP_FAMILY`

mod controlled;
mod lines;
mod modules;
mod passive;
mod semi;

use super::codes::Row;
use super::expr_read::{self, Fail, Tree};
use super::number::{self, Scan};
use super::read::{Lower, Stmt};
use super::tables::{self, Eval};
use crate::ir::{self, CircuitExtra as X, ElemKind as K, Feature, FeatureSet, ItemKind};

/// What the element area reads: every kind below, and the field members its rules produce.
/// Not: `MultOther` (no switch, line or coupling of ngspice takes `m`), `MultInstanceParallel`
/// (an X `m` is ngspice's append law, R31), `TcBaseTnom` (a B source measures from 27 °C),
/// `NameWithoutLetter` (A13), `BuiltinMaster`, `IcInOp`, `SemiInitialOn` (a semiconductor takes `off`
/// only), parasitics,
/// Laplace, frequency tables, delays, ports, modules named directly, unresolved masters.
pub(super) const FEATURES: FeatureSet = tables::feature_set(&[
    Feature::Item(ItemKind::Element),
    Feature::Elem(K::Resistor),
    Feature::Elem(K::Capacitor),
    Feature::Elem(K::Inductor),
    Feature::Elem(K::ChargeCapacitor),
    Feature::Elem(K::Coupling),
    Feature::Elem(K::Vcvs),
    Feature::Elem(K::Vccs),
    Feature::Elem(K::Cccs),
    Feature::Elem(K::Ccvs),
    Feature::Elem(K::Poly),
    Feature::Elem(K::Table),
    Feature::Elem(K::Behav),
    // a B source reads a parameter untyped (A33, R56)
    Feature::Circuit(X::BehavUntypedParam),
    Feature::Elem(K::Diode),
    Feature::Elem(K::Bjt),
    Feature::Elem(K::Mosfet),
    Feature::Elem(K::Jfet),
    Feature::Elem(K::Mesfet),
    Feature::Elem(K::VSwitch),
    Feature::Elem(K::ISwitch),
    Feature::Elem(K::LosslessLine),
    Feature::Elem(K::LossyLine),
    Feature::Elem(K::RcLine),
    Feature::Elem(K::SingleLossyLine),
    Feature::Elem(K::CoupledLines),
    Feature::Elem(K::CodeModel),
    Feature::Elem(K::ModuleModel),
    Feature::Elem(K::Instance),
    Feature::Circuit(X::MultResistor),
    Feature::Circuit(X::MultCapacitor),
    Feature::Circuit(X::MultInductor),
    // `m` on G, F and a current B (R58); an E's or H's multiplies a voltage: not this member
    Feature::Circuit(X::MultControlledCurrent),
    Feature::Circuit(X::MultSemi),
    Feature::Circuit(X::MultInstanceNgspiceAppend),
    // an N's `m=` (settle osdi_mult.sp); an A card takes none (admits refuses one)
    Feature::Circuit(X::MultModule),
    Feature::Circuit(X::ElementWithoutModel),
    Feature::Circuit(X::BehavTc),
    Feature::Circuit(X::TcBaseFixed27),
    Feature::Circuit(X::TcLawDirect),
    Feature::Circuit(X::TcLawReciprocal),
    Feature::Circuit(X::IcUicOnly),
    Feature::Circuit(X::PassiveDynamicValue),
    Feature::Circuit(X::PassiveNoiseless),
    Feature::Circuit(X::PassiveAc),
    Feature::Circuit(X::PassiveScale),
    Feature::Circuit(X::PassiveTc),
    Feature::Circuit(X::PassiveTce),
    Feature::Circuit(X::InstanceTemp),
    Feature::Circuit(X::InstanceDtemp),
    Feature::Circuit(X::CouplingMany),
    Feature::Circuit(X::TableNgspiceRounded),
    Feature::Circuit(X::SemiArea),
    Feature::Circuit(X::SemiInitialOff),
    Feature::Circuit(X::DiodeThermal),
    Feature::Circuit(X::SwitchInitial),
    Feature::Circuit(X::LineFreqNl),
    Feature::Circuit(X::LineLumps),
])
// elements in a subcircuit (manual §2.4.3), a `.lib` section (run: a section's `R` conducts)
// and an `.if` branch (manual §2.15; run: a nested branch's `R` in a subcircuit conducts)
.union(FeatureSet::placed(
    ir::ScopeKind::Subckt,
    &[ItemKind::Element],
))
.union(FeatureSet::placed(
    ir::ScopeKind::Section,
    &[ItemKind::Element],
))
.union(FeatureSet::placed(
    ir::ScopeKind::Branch,
    &[ItemKind::Element],
));

/// An element card ngspice cannot read: too few nodes, no master, a value or parameter its
/// device does not take (`unknown parameter`), a line it ignores: `InvalidInSource`.
pub(super) const MALFORMED: ir::Code = ir::Code::new(b'P', 1100);
/// An element form ngspice reads that this reader has no rule for yet: `Unread`.
pub(super) const UNREAD_FORM: ir::Code = ir::Code::new(b'P', 1101);
/// A parameter ngspice drops when it runs an R, C or L whose value reads the circuit as a B
/// source (`inpcom.c` keeps the value, `m`, `tc1`, `tc2` and the noise switch): left out.
pub(super) const DROPPED: ir::Code = ir::Code::new(b'P', 1102);
/// `tc1`/`tc2` the IR cannot state: on an R/C/L run as a B source whose value reads only the
/// temperature (`TcBase::Fixed27` holds only on a circuit-valued row), on a charge capacitor
/// (ngspice divides its charge by the factor), or a number ngspice rounds to 9 digits
/// (`inpcom.c` `eval_tc`, `%15.8e`): `Unread`.
pub(super) const TC_BASE: ir::Code = ir::Code::new(b'P', 1103);
/// An R/C/L run as a B source whose first `{…}` on the line is not its value: ngspice runs that
/// expression as the value (`inpcom.c` `strchr(cut_line, '{')`, INV 14.13): `Private`.
pub(super) const FIRST_BRACE: ir::Code = ir::Code::new(b'P', 1105);
/// An ngspice-only form: XSPICE port modifiers (`%v`, `[ ]`), `reciproctc`, `reciprocm`:
/// `Private`.
pub(super) const PRIVATE_FORM: ir::Code = ir::Code::new(b'P', 1106);
/// A key a card gives twice: ngspice keeps the last (runs: `R1 a 0 1k r=2k` is 2 kΩ,
/// `tc1=0.01 tc1=0.02` is 0.02, `X1 … w=1 w=2` passes 2); the earlier ones are left out.
pub(super) const DUPLICATE_KEY: ir::Code = ir::Code::new(b'P', 1107);
/// An X card whose node count is not the port count of the subcircuit it names (R61, A36):
/// ngspice rejects the deck (settle `instance_ports_*.sp`): `InvalidInSource`.
pub(super) const PORT_COUNT: ir::Code = ir::Code::new(b'P', 1108);
/// An R, C or L card with a model whose value stands where ngspice-45 rejects it: before the
/// model (a C's or L's, or an R's braced or quoted value), or an L's `l=` (settle
/// `passive_model_order.sp`): `InvalidInSource`.
pub(super) const MODEL_ORDER: ir::Code = ir::Code::new(b'P', 1109);

/// An instance `temp=`/`dtemp=` on an element whose bound model's family has none on ngspice
/// ("unknown parameter": runs fam, `instance_temp`): the deck is invalid in its source.
pub(super) const TEMP_FAMILY: ir::Code = ir::Code::new(b'R', 1100);

/// This area's codes.
#[rustfmt::skip] // a table: one row per line
pub(super) const CODES: &[Row] = &[
    row(MALFORMED, "an element card ngspice cannot read", "ngspice-45 spicelib/parser/inp2*.c", "too few nodes, no master, or a value or parameter the device does not take; kept as text (InvalidInSource)."),
    row(UNREAD_FORM, "an element form the ngspice reader has no rule for yet", "rewrite/DIALECT-NGSPICE.md §3", "kept as ngspice text (Unread); a strict read refuses it."),
    row(DROPPED, "a parameter ngspice drops on an R, C or L it runs as a B source", "ngspice-45 frontend/inpcom.c inp_compat; rewrite/INV-semantics.md 6.14", "only the value, m, tc1, tc2 and the noise switch survive the rewrite; the rest is left out."),
    row(TC_BASE, "a behavioural temperature coefficient the IR cannot state", "ngspice-45 frontend/inpcom.c eval_tc; spicelib/devices/asrc/asrcload.c", "on a temperature-only or charge-defined R/C/L run as a B source, or rounded by ngspice to 9 digits (Unread)."),
    row(FIRST_BRACE, "an R, C or L whose first braced expression is not its value", "ngspice-45 frontend/inpcom.c inp_compat; rewrite/INV-semantics.md 14.13", "ngspice runs the first {...} on the line as the value; kept as ngspice text (Private)."),
    row(DUPLICATE_KEY, "a parameter given twice on an element card", "ngspice-45 INPdevParse (each value set in turn); runs", "ngspice keeps the last value; the earlier ones are left out."),
    row(PORT_COUNT, "a subcircuit instance whose node count is not its subcircuit's port count", "ngspice-45 frontend/subckt.c; settle instance_ports_short.sp, instance_ports_long.sp", "ngspice rejects the deck; kept as text (InvalidInSource)."),
    row(MODEL_ORDER, "an R, C or L value where ngspice does not take it beside a model", "ngspice-45 inp2r.c, inp2c.c, inp2l.c; settle passive_model_order.sp", "before the model only an R's number is taken, and an L with a model takes no l=; ngspice stops on 'unknown parameter'; kept as text (InvalidInSource)."),
    row(TEMP_FAMILY, "an instance temperature on a model family that has none", "ngspice-45 device IFparm tables (bsim3/4, b3soi, b4soi, mes: no temp or dtemp; hicum2, soi3: no dtemp; hisimhv, mesa: no temp); runs", "ngspice rejects the card (unknown parameter); the deck is invalid in its source."),
    row(PRIVATE_FORM, "an ngspice-only element form", "ngspice manual ch.12 (XSPICE port modifiers); asrc.c reciproctc, reciprocm", "kept as ngspice text (Private): no other simulator has it."),
];

/// One item of an element card after its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct Item {
    /// What it is.
    pub(super) kind: Kind,
    /// A word's text, a parameter's key, a group's inner text.
    pub(super) text: String,
    /// A parameter's value as written (its braces kept); empty for the other kinds.
    pub(super) value: String,
    /// Its first byte in the card text.
    pub(super) at: usize,
    /// Whether a comma stood before it.
    pub(super) comma: bool,
}

/// The kinds of [`Item`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Kind {
    /// A bare word (a node, a name, a number, a flag, `poly(2)`, `=`).
    Word,
    /// `key=value`.
    Assign,
    /// `(…)`.
    Group,
    /// `{…}` or `'…'`.
    Brace,
}

/// A value read but not appended: a card number or an expression tree.
#[derive(Clone, Debug)]
pub(super) enum Val {
    /// A card number (`INPevaluate`).
    Lit(Scan),
    /// A braced expression.
    Tree(Tree),
}

/// An element header before it is appended: names as written.
#[derive(Debug, Default)]
pub(super) struct Card {
    /// The element's name, letter included.
    pub(super) name: String,
    /// Its nodes in the kind's role order.
    pub(super) nodes: Vec<String>,
    /// Its master's name.
    pub(super) model: Option<String>,
    /// Instance parameters with no typed field, keys as written.
    pub(super) params: Vec<(String, Val)>,
    /// `m`.
    pub(super) mult: Option<Val>,
    /// Whether a key was given twice (ngspice keeps the last; [`head`] notes it).
    pub(super) dups: bool,
}

/// An element card (the spine's dispatch: every letter but `V` and `I`).
pub(super) fn element(lw: &mut Lower<'_, '_>, st: Stmt) {
    let name = lw.toks.text(lw.ast.main_token(st.node)).to_string();
    let text = lw.rest(st.node);
    let Some(items) = items(&text) else {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    };
    let letter = name.as_bytes()[0].to_ascii_lowercase();
    let card = Card {
        name,
        ..Card::default()
    };
    match letter {
        b'r' | b'c' | b'l' => passive::rcl(lw, st, card, &items, &text),
        b'k' => passive::coupling(lw, st, card, &items),
        b'd' | b'q' | b'm' | b'j' | b'z' => semi::semi(lw, st, card, &items),
        b's' | b'w' => semi::switch(lw, st, card, &items),
        b'e' | b'g' | b'f' | b'h' => controlled::efgh(lw, st, card, &items, &text),
        b'b' => controlled::b(lw, st, card, &items, &text),
        b't' | b'o' | b'u' | b'y' | b'p' => lines::line(lw, st, card, &items),
        b'x' => modules::x(lw, st, card, &items),
        b'n' => modules::n(lw, st, card, &items),
        b'a' => modules::a(lw, st, card, &items),
        _ => lw.unread(st),
    }
}

impl Card {
    /// Sets instance parameter `key` to `v`: a later value replaces an earlier one (ngspice
    /// keeps the last) and marks the card.
    pub(super) fn param(&mut self, key: &str, v: Val) {
        if let Some(p) = self
            .params
            .iter_mut()
            .find(|p| p.0.eq_ignore_ascii_case(key))
        {
            p.1 = v;
            self.dups = true;
        } else {
            self.params.push((key.to_string(), v));
        }
    }
}

impl Item {
    /// The item's text as a value: a word as written, a brace with its braces.
    pub(super) fn as_value(&self) -> String {
        match self.kind {
            Kind::Brace => format!("{{{}}}", self.text),
            _ => self.text.clone(),
        }
    }
}

/// A value as written (`1k`, `-2`, `{w*2}`, `'w*2'`, a bare parameter name `rval`): a card
/// number, or the expression read by the evaluator its class picks (R14). numparam substitutes a
/// bare name after `=` (run: `.param rval=2k`, `R1 a b r=rval` is 2 kΩ; gf180's `tc1=trx1`).
pub(super) fn val(lw: &Lower<'_, '_>, st: Stmt, text: &str) -> Result<Val, Fail> {
    let t = text.trim();
    let ident = t
        .bytes()
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == b'_')
        && t.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_');
    if let Some(inner) = braced(text).or(ident.then_some(t)) {
        let class = lw.class(st.container, inner, &[]);
        let eval = if class >= super::declare::Class::Temp {
            Eval::Behavioural
        } else {
            Eval::Numparam
        };
        return expr_read::parse_in(inner, eval).map(Val::Tree);
    }
    number_word(text).map(Val::Lit).ok_or_else(Fail::invalid)
}

/// A B-source field (`v=`, `i=`, `vol=`, `cur=`, `value=`, a table's input, an R/C/L value run
/// as a B source): the B-source parser's, whatever its class (`inp_bsource_compat`).
pub(super) fn b_tree(text: &str) -> Result<Tree, Fail> {
    let inner = braced(text).unwrap_or(text.trim());
    if inner.is_empty() {
        return Err(Fail::invalid());
    }
    expr_read::parse_in(inner, Eval::Behavioural)
}

/// Appends a value read by [`val`].
pub(super) fn emit(lw: &mut Lower<'_, '_>, st: Stmt, v: &Val) -> ir::ExprId {
    match v {
        Val::Lit(s) => lw.number(s.literal, &s.fit),
        Val::Tree(t) => lw.expr(st, t, st.scope),
    }
}

/// Appends `card`'s header in the order the IR text reads it back: the name, the terminals, the
/// master, the parameters (values, then keys), the multiplicity.
pub(super) fn head(lw: &mut Lower<'_, '_>, st: Stmt, card: &Card) -> ir::ElemHead {
    if card.dups {
        lw.note(st, DUPLICATE_KEY);
    }
    let name = lw.names.fold(&card.name);
    let name = lw.b.name(&name);
    let nodes: Vec<String> = card.nodes.iter().map(|n| lw.names.node(n)).collect();
    let terminals = if nodes.is_empty() {
        ir::List::EMPTY
    } else {
        let refs: Vec<&str> = nodes.iter().map(String::as_str).collect();
        lw.b.names(&refs)
    };
    let model = match &card.model {
        Some(m) => {
            let m = lw.names.fold(m);
            ir::Ref::unresolved(lw.b.name(&m))
        }
        None => ir::Ref::unresolved(ir::NameId::NONE),
    };
    let values: Vec<ir::ExprId> = card.params.iter().map(|(_, v)| emit(lw, st, v)).collect();
    let params = if values.is_empty() {
        ir::List::EMPTY
    } else {
        let keys: Vec<String> = card.params.iter().map(|(k, _)| lw.names.fold(k)).collect();
        let pairs: Vec<(&str, ir::ExprId)> = keys.iter().map(String::as_str).zip(values).collect();
        lw.b.params(&pairs)
    };
    let mult = card
        .mult
        .as_ref()
        .map_or(ir::ExprId::NONE, |m| emit(lw, st, m));
    ir::ElemHead {
        name,
        terminals,
        model,
        params,
        mult,
        mult_law: ir::MultLaw::Parallel,
    }
}

/// Queues the master of a pushed element for the bind step (R3): slot 0, the kind's master
/// definition kind.
pub(super) fn push_done(lw: &mut Lower<'_, '_>, st: Stmt, item: ir::ItemRef, card: &Card) {
    let e: ir::Element = lw.b.design().row(item.index);
    if let (Some(_), Some(kind)) = (&card.model, e.kind.master_kind()) {
        lw.refer(item, 0, st.scope, kind, e.model.name);
    }
}

/// A word that is a card number (`INPevaluate` with gobble: `1k`, `-2.5`, `10v`).
pub(super) fn number_word(text: &str) -> Option<Scan> {
    let t = text.trim();
    let first = t.bytes().find(|c| *c != b'-' && *c != b'+')?;
    (first.is_ascii_digit() || first == b'.')
        .then(|| number::card(t))
        .flatten()
}

/// Whether the family of `e`'s bound model takes an instance `temp` and `dtemp` on ngspice-45
/// (the device's instance `IFparm` table; runs fam: `temp=40`/`dtemp=40` on an instance of each
/// family, "unknown parameter" where it is false). `(true, true)` when the master is unbound or
/// its family is not a semiconductor's of this table. The writer's `admits` and the reader's
/// bound check read this one table.
pub(super) fn instance_temp(design: &ir::Design, e: &ir::Element) -> (bool, bool) {
    use ir::ModelFamily as F;
    if e.model.def.is_none() {
        return (true, true);
    }
    let item = design.def(e.model.def).item;
    if item.kind != ItemKind::Model {
        return (true, true);
    }
    let m: ir::Model = design.row(item.index);
    match m.family {
        // bsim1, bsim2, bsim3*, bsim4*, bsimsoi (b4soi.c), bsim3soi_*, mes: neither
        F::Bsim1
        | F::Bsim2
        | F::Bsim3
        | F::Bsim4
        | F::B4Soi
        | F::B3SoiFd
        | F::B3SoiDd
        | F::B3SoiPd
        | F::Mesfet => (false, false),
        // hicum2.c, soi3.c: temp only
        F::Hicum2 | F::Soi3 => (true, false),
        // hsmhv.c, hsmhv2.c, mesa.c: dtemp only
        F::HisimHv | F::Mesa => (false, true),
        // dio, bjt, vbic, mos1-9, hsm2, vdmos, jfet, jfet2, hfet1, hfet2: both
        _ => (true, true),
    }
}

/// The bind step's check of this area (called by `read.rs` after `bind_rows`): a coded row
/// ([`TEMP_FAMILY`]) at every element whose instance temperature its bound family lacks.
#[allow(
    dead_code,
    reason = "read.rs calls it after bind_rows once ng-scaffold adds the call"
)]
pub(super) fn bound(design: &ir::Design, diags: &mut ir::Diags) {
    for (item, _) in design.reachable() {
        if item.kind != ItemKind::Element {
            continue;
        }
        let e: ir::Element = design.row(item.index);
        if e.kind.detail() != ir::DetailKind::Semi {
            continue;
        }
        let s = design.semi(&e);
        let (temp, dtemp) = instance_temp(design, &e);
        if !temp && !s.temp_abs.is_none() || !dtemp && !s.dtemp.is_none() {
            diags.push(TEMP_FAMILY, design.span_of(item), 0);
        }
    }
}

/// A literal by its parts.
pub(super) const fn literal(mantissa: i64, frac: u8, exp10: i16, form: ir::NumForm) -> Scan {
    Scan {
        literal: ir::Literal {
            mantissa,
            frac,
            exp10,
            form,
        },
        len: 0,
        fit: number::Fit::Exact,
    }
}

// ---- private ----

/// A note row of this area.
const fn row(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    let kind = ir::Kind::Note;
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind,
        },
    )
}

/// The inner text of a value wrapped whole in `{…}` or `'…'`.
fn braced(text: &str) -> Option<&str> {
    let t = text.trim();
    if let Some(inner) = t.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')) {
        return (!inner.contains('\'')).then_some(inner);
    }
    let inner = t.strip_prefix('{')?.strip_suffix('}')?;
    let mut depth = 0i32;
    let whole = inner.bytes().all(|c| {
        depth += i32::from(c == b'{') - i32::from(c == b'}');
        depth >= 0
    });
    (whole && depth == 0).then_some(inner)
}

/// The card text after the name, split into items; `None` when a group does not close or a
/// parameter has no value.
fn items(text: &str) -> Option<Vec<Item>> {
    let b = text.as_bytes();
    let mut out = Vec::new();
    let (mut i, mut comma) = (0, false);
    // Bounded: each turn moves `i` forward by at least one byte.
    while i < b.len() {
        let at = i;
        let (kind, txt, value) = match b[i] {
            b' ' | b'\t' | b'\r' | b'\n' => {
                i += 1;
                continue;
            }
            b',' => {
                comma = true;
                i += 1;
                continue;
            }
            b'{' | b'\'' => {
                i = group_end(b, i)?;
                (Kind::Brace, text[at + 1..i - 1].to_string(), String::new())
            }
            b'(' => {
                i = group_end(b, i)?;
                (Kind::Group, text[at + 1..i - 1].to_string(), String::new())
            }
            b'=' => {
                i += 1;
                (Kind::Word, "=".to_string(), String::new())
            }
            _ => {
                i = word_end(b, i)?;
                let word = text[at..i].to_string();
                let mut j = i;
                while j < b.len() && b[j] == b' ' {
                    j += 1;
                }
                if j < b.len() && b[j] == b'=' && b.get(j + 1) != Some(&b'=') {
                    j += 1;
                    while j < b.len() && b[j] == b' ' {
                        j += 1;
                    }
                    if j >= b.len() {
                        return None;
                    }
                    let end = match b[j] {
                        b'{' | b'\'' | b'(' => group_end(b, j)?,
                        _ => word_end(b, j)?,
                    };
                    if end == j {
                        return None;
                    }
                    i = end;
                    (Kind::Assign, word, text[j..end].to_string())
                } else {
                    (Kind::Word, word, String::new())
                }
            }
        };
        out.push(Item {
            kind,
            text: txt,
            value,
            at,
            comma,
        });
        comma = false;
    }
    Some(out)
}

/// The end of a word starting at `i`: up to a blank, comma, `=`, `{` or `'`, with any `(…)` it
/// touches.
fn word_end(b: &[u8], mut i: usize) -> Option<usize> {
    // Bounded: each turn moves `i` forward.
    while i < b.len() {
        match b[i] {
            b' ' | b'\t' | b'\r' | b'\n' | b',' | b'=' | b'{' | b'\'' => break,
            b'(' => i = group_end(b, i)?,
            _ => i += 1,
        }
    }
    Some(i)
}

/// The byte after the group opened at `i` (`{…}` and `(…)` nest; `'…'` does not).
fn group_end(b: &[u8], i: usize) -> Option<usize> {
    let (open, close) = match b[i] {
        b'{' => (b'{', b'}'),
        b'(' => (b'(', b')'),
        _ => (0, b'\''),
    };
    let mut depth = 0usize;
    for (k, &c) in b.iter().enumerate().skip(i + 1) {
        if c == close && depth == 0 {
            return Some(k + 1);
        }
        if open != 0 && c == open {
            depth += 1;
        } else if open != 0 && c == close {
            depth -= 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{Kind, items};

    #[test]
    fn items_split_by_ngspice_token_rules() {
        let got = items("a b poly(2) k = 1 ic=1, 2 {x*2} 'y' (0,1)").expect("items");
        let kinds: Vec<Kind> = got.iter().map(|i| i.kind).collect();
        assert_eq!(
            kinds,
            [
                Kind::Word,
                Kind::Word,
                Kind::Word,
                Kind::Assign,
                Kind::Assign,
                Kind::Word,
                Kind::Brace,
                Kind::Brace,
                Kind::Group
            ]
        );
        assert_eq!(got[2].text, "poly(2)");
        assert_eq!((got[3].text.as_str(), got[3].value.as_str()), ("k", "1"));
        assert!(got[5].comma, "a comma before the second ic value");
        assert_eq!(got[6].text, "x*2");
        assert!(items("a {b").is_none(), "an unclosed brace");
        assert!(items("a k=").is_none(), "a parameter with no value");
    }
}
