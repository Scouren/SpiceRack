//! Independent source cards (S3.1b): a `V` or `I` card → one `Element` of kind `VSource`,
//! `ISource` or `Port` with its `Source` row, side rows and waveform row.
//!
//! The card's parts come from `card.rs` (ngspice's device parser: a leading value, then
//! keywords in any order, the later winning). Their meaning, from ngspice-45:
//!
//! - DC: the leading value or `dc` (`c` on an `I` card); `dc` with no number is 0. With a wave
//!   and no DC, the operating point is the wave's law at t = 0 for every wave (`vsrcload.c`
//!   MODEDC: time = 0, the same switch): `DcRule::WaveAtZero` (R36). With neither, ngspice says
//!   "has no value, DC 0 assumed" (`vsrctemp.c`): `Given` 0 with a note (INV 5.14 was wrong).
//! - AC: `ac [mag [phase]]`, `acmag`, `acphase`; a missing magnitude is 1, a missing phase 0
//!   (`vsrctemp.c`); `distof1`/`distof2` `mag [phase]`, phase 0 when absent (`vsrcpar.c`).
//! - `m` on an `I` card multiplies every slot (`isrcload.c` `m * value`): the head's `mult`.
//! - `portnum n [z0 z]` (RFSPICE is in the pinned binary): an `ElemKind::Port`, `z0` 50 when
//!   absent (`vsrctemp.c`); `pwr`, `freq`, `phase` are `Unread` (A17 Q4).
//! - Waves (A17, R35): every field holds what the card holds, `NONE` when omitted; the wave's
//!   `GivenZero::SpiceDefault` mode states ngspice's defaults and its zero rule. SIN is
//!   `HoldDelayValue` before its delay (`vsrcload.c`: VO + VA·sin(phase)); SFFM, AM `Zero`; an
//!   `I` card's SFFM has no delay and reads its 6th value as the modulation phase and its 7th
//!   as the carrier phase (`isrcload.c` SFFM; run), so it is `HoldDelayValue`. Values past a
//!   wave's arity are ignored by ngspice: dropped with a note.
//! - Values: a number (`INPevaluate`), or an expression numparam evaluates (`{…}`, `'…'`, a
//!   bare parameter name). One that reads the circuit or the temperature is ngspice's fatal
//!   "Undefined parameter" (runs `{time*1}`, `{temper}`): `InvalidInSource`.
//!
//! Every statement becomes one element or one `Opaque` with a coded note; the whole card is
//! validated, every expression parsed, before the first push.
//!
//! Cites: ngspice-45 `inp2v.c`, `inp2i.c`, `inpdpar.c`, `inpgval.c`, `vsrc.c`, `isrc.c`,
//! `vsrcpar.c`, `vsrctemp.c`, `vsrcload.c`, `isrcload.c`, `1-f-code.c`; `rewrite/RULINGS.md`
//! R35, R36; `rewrite/AMEND-A12.md` A17; `rewrite/INV-semantics.md` 5.01–5.21.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! the codes `SOURCE_INVALID`, `SOURCE_DYNAMIC`, `SOURCE_EXTERNAL`, `PORT_DRIVE`, `RANDOM_TYPE`,
//! `PWL_REPEAT`, `PORT_NUMBER`, `NO_VALUE`, `EXTRA_VALUES`, `REPLACED`, `IGNORED`;
//! `pub(super) enum Val`, `pub(super) fn source(lw: &mut Lower<'_, '_>, st: Stmt)`,
//! `pub(super) fn val`, `emit`, `emit_opt`, `int`, `unread`, `invalid` (the wave reader's)

mod card;
mod wave;

use super::codes::Row;
use super::declare::Class;
use super::expr_read::{self, Fail, Tree};
use super::number;
use super::read::{Lower, Stmt};
use super::tables::{self, Eval};
use crate::ir;
use crate::ir::Feature as F;
use crate::ir::circuit_extra::CircuitExtra as X;
use card::Bad;

/// What this area's reader produces (its writer spells the same set, law L5).
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    F::Item(ir::ItemKind::Element),
    F::Elem(ir::ElemKind::VSource),
    F::Elem(ir::ElemKind::ISource),
    F::Elem(ir::ElemKind::Port),
    F::Wave(ir::WaveKind::Pulse),
    F::Wave(ir::WaveKind::Sin),
    F::Wave(ir::WaveKind::Exp),
    F::Wave(ir::WaveKind::Pwl),
    F::Wave(ir::WaveKind::Sffm),
    F::Wave(ir::WaveKind::Am),
    F::Wave(ir::WaveKind::TrNoise),
    F::Wave(ir::WaveKind::TrRandom),
    F::Circuit(X::ElementWithoutModel),
    F::Circuit(X::GivenZeroSpiceDefault),
    F::Circuit(X::MultISource),
    F::Circuit(X::SourceDistof),
    F::Circuit(X::PulseSpice),
    F::Circuit(X::PulseCycles),
    F::Circuit(X::SinPreHold),
    F::Circuit(X::ExpTd2FromZero),
    F::Circuit(X::SffmNgspice),
    F::Circuit(X::SffmPreZero),
    F::Circuit(X::SffmPreHold),
    F::Circuit(X::AmPreZero),
    F::Circuit(X::PwlDelay),
    F::Circuit(X::PwlRepeatFrom),
    F::Circuit(X::DcGivenWithWave),
    F::Circuit(X::DcAtZeroPulse),
    F::Circuit(X::DcAtZeroSin),
    F::Circuit(X::DcAtZeroExp),
    F::Circuit(X::DcAtZeroPwl),
    F::Circuit(X::DcAtZeroSffm),
    F::Circuit(X::DcAtZeroAm),
    F::Circuit(X::DcAtZeroTrNoise),
    F::Circuit(X::DcAtZeroTrRandom),
]);

/// A card ngspice rejects: fewer than two nodes, a stray value (`V1 a 0 1 2`), a keyword its
/// device has not (`m` on a `V`, `r`/`td` on an `I`), a wrong value count, a PWL with an odd
/// count (ngspice reads past its list), a PWL `r` that is no listed time before the last.
pub(super) const SOURCE_INVALID: ir::Code = ir::Code::new(b'P', 1150);
/// A source value that reads the circuit, the time or the temperature.
pub(super) const SOURCE_DYNAMIC: ir::Code = ir::Code::new(b'P', 1151);
/// `external`.
pub(super) const SOURCE_EXTERNAL: ir::Code = ir::Code::new(b'P', 1152);
/// A port's `pwr`, `freq` or `phase`.
pub(super) const PORT_DRIVE: ir::Code = ir::Code::new(b'P', 1153);
/// A `TRRANDOM` type that is no literal, or outside 1–4 after ngspice's truncation.
pub(super) const RANDOM_TYPE: ir::Code = ir::Code::new(b'P', 1154);
/// A PWL `r` the IR cannot state: an expression, or one given before the PWL it repeats.
pub(super) const PWL_REPEAT: ir::Code = ir::Code::new(b'P', 1155);
/// A `portnum` that is no positive integer literal, or a `z0` literal at or below 0.
pub(super) const PORT_NUMBER: ir::Code = ir::Code::new(b'P', 1156);
/// A card with no DC value and no wave: DC 0.
pub(super) const NO_VALUE: ir::Code = ir::Code::new(b'P', 1157);
/// Values past a wave's arity, ignored by ngspice and dropped.
pub(super) const EXTRA_VALUES: ir::Code = ir::Code::new(b'P', 1158);
/// A keyword given twice, a second wave, or a leading value and `dc`: the later holds.
pub(super) const REPLACED: ir::Code = ir::Code::new(b'P', 1159);
/// A keyword ngspice ignores here: `td` or `r` with no PWL, an `r` below −0.5 (no repeat).
pub(super) const IGNORED: ir::Code = ir::Code::new(b'P', 1160);

/// This area's codes.
pub(super) const CODES: &[Row] = &[
    row(
        SOURCE_INVALID,
        "a source card ngspice rejects",
        "ngspice-45 inpdpar.c, inpgval.c, vsrc.c, isrc.c, vsrcpar.c",
        "a missing node, a stray value, a keyword the device has not (m on a V, r or td on an I), a wrong value count, an odd PWL count or a PWL r that is no listed time before the last; kept as text (InvalidInSource).",
    ),
    row(
        SOURCE_DYNAMIC,
        "a source value that reads the circuit or the temperature",
        "ngspice-45 numparam; runs {time*1}, {temper}",
        "numparam evaluates a source value once and has no time, v(), i() or temper: ngspice stops with \"Undefined parameter\"; kept as text (InvalidInSource).",
    ),
    row(
        SOURCE_EXTERNAL,
        "an external source",
        "ngspice-45 vsrc.c (#ifdef SHARED_MODULE)",
        "the pinned ngspice binary has no external sources (\"unknown parameter\"); kept as text (InvalidInSource).",
    ),
    row(
        PORT_DRIVE,
        "a port with pwr, freq or phase",
        "ngspice-45 vsrcpar.c, vsrcload.c; rewrite/AMEND-A12.md A17",
        "the IR's port has no phase; pwr and freq replace the wave or not by their order against the wave keyword; the port drive adds to a value ngspice never sets; kept as text (Unread).",
    ),
    row(
        RANDOM_TYPE,
        "a TRRANDOM type the IR cannot state",
        "ngspice-45 vsrcpar.c ((int) of the value), 1-f-code.c",
        "an expression, or a type outside 1-4 after truncation; kept as text (Unread).",
    ),
    row(
        PWL_REPEAT,
        "a PWL r the IR cannot state",
        "ngspice-45 vsrcpar.c VSRC_R",
        "an expression (below -0.5 it means no repeat), or an r given before its PWL (ngspice checks it against the values it holds then); kept as text (Unread).",
    ),
    row(
        PORT_NUMBER,
        "a port number or impedance the IR cannot state",
        "ngspice-45 vsrcpar.c, vsrctemp.c",
        "portnum is rounded and is a port only above 0, and z0 at or below 0 is no port; kept as text (Unread).",
    ),
    row(
        NO_VALUE,
        "a source with no value",
        "ngspice-45 vsrctemp.c (\"has no value, DC 0 assumed\")",
        "read as DC 0.",
    ),
    row(
        EXTRA_VALUES,
        "waveform values past the wave's arity",
        "ngspice-45 vsrcload.c, isrcload.c",
        "ngspice ignores them; they are dropped.",
    ),
    row(
        REPLACED,
        "a source keyword given twice",
        "ngspice-45 inpdpar.c, inp2v.c",
        "the later value holds (a leading value holds over dc); the earlier is dropped.",
    ),
    row(
        IGNORED,
        "a source keyword ngspice ignores",
        "ngspice-45 vsrcpar.c, vsrcload.c",
        "td or r on a source whose wave is no PWL, or r below -0.5 (no repeat); dropped.",
    ),
];

/// A value read and not yet pushed.
#[derive(Clone, Debug)]
pub(super) enum Val {
    /// A number, and whether its digits were rounded (A3).
    Lit(ir::Literal, bool),
    /// An expression numparam evaluates.
    Tree(Tree),
}

/// A card's values, parsed: everything the push needs.
struct Read {
    dc: Option<Val>,
    ac: Option<(Val, Val)>,
    distof: [Option<(Val, Val)>; 2],
    mult: Option<Val>,
    port: Option<(Val, Val)>,
    wave: Option<wave::Wave>,
    notes: Vec<ir::Code>,
}

/// A `V` or `I` card.
pub(super) fn source(lw: &mut Lower<'_, '_>, st: Stmt) {
    let head = lw.toks.text(lw.ast.main_token(st.node)).to_string();
    let current = head.as_bytes()[0].eq_ignore_ascii_case(&b'i');
    let rest = lw.rest(st.node);
    let c = match card::card(&rest, current) {
        Ok(c) => c,
        Err(Bad::Invalid) => {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, SOURCE_INVALID);
        }
        Err(Bad::External) => {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, SOURCE_EXTERNAL);
        }
    };
    if c.drive {
        return lw.opaque(st, ir::OpaqueClass::Unread, PORT_DRIVE);
    }
    let read = match read_card(lw, st, &c) {
        Ok(r) => r,
        Err(f) => return lw.opaque(st, f.class, f.code),
    };
    for &code in &read.notes {
        lw.note(st, code);
    }
    push(lw, st, &head, &c.nodes, read);
}

/// A value token: a number, or an expression numparam evaluates (its `{…}`/`'…'` dropped).
pub(super) fn val(lw: &Lower<'_, '_>, st: Stmt, tok: &str) -> Result<Val, Fail> {
    if let Some(s) = number::card(tok) {
        return Ok(Val::Lit(s.literal, s.rounded));
    }
    let text = unwrap(tok);
    if lw.class(st.container, text, &[]) != Class::Constant {
        return Err(Fail {
            code: SOURCE_DYNAMIC,
            class: ir::OpaqueClass::InvalidInSource,
        });
    }
    expr_read::parse_in(text, Eval::Numparam).map(Val::Tree)
}

/// Appends a value.
pub(super) fn emit(lw: &mut Lower<'_, '_>, st: Stmt, v: &Val) -> ir::ExprId {
    match v {
        Val::Lit(lit, rounded) => {
            if *rounded {
                lw.note(st, super::codes::ROUNDED);
            }
            lw.b.literal(*lit)
        }
        Val::Tree(t) => lw.expr(st, t, st.scope),
    }
}

/// Appends an optional value (`NONE` when absent).
pub(super) fn emit_opt(lw: &mut Lower<'_, '_>, st: Stmt, v: Option<&Val>) -> ir::ExprId {
    v.map_or(ir::ExprId::NONE, |v| emit(lw, st, v))
}

/// A plain integer literal (a default ngspice gives: AC magnitude 1, `z0` 50, …).
pub(super) fn int(n: i64) -> Val {
    let lit = if n == 0 {
        ir::Literal::ZERO
    } else {
        ir::Literal {
            mantissa: n,
            frac: 0,
            exp10: 0,
            form: ir::NumForm::Plain,
        }
    };
    Val::Lit(lit, false)
}

/// An `Unread` failure with `code`.
pub(super) fn unread(code: ir::Code) -> Fail {
    Fail {
        code,
        class: ir::OpaqueClass::Unread,
    }
}

/// An `InvalidInSource` failure with `code`.
pub(super) fn invalid(code: ir::Code) -> Fail {
    Fail {
        code,
        class: ir::OpaqueClass::InvalidInSource,
    }
}

// ---- private ----

/// Every value of the card parsed, or why the card is an `Opaque`.
fn read_card(lw: &Lower<'_, '_>, st: Stmt, c: &card::Card<'_>) -> Result<Read, Fail> {
    let opt = |v: Option<Option<&str>>, absent: i64| -> Result<Option<Val>, Fail> {
        v.map(|v| v.map_or(Ok(int(absent)), |t| val(lw, st, t)))
            .transpose()
    };
    let mut notes = Vec::new();
    if c.replaced {
        notes.push(REPLACED);
    }
    let wave = match &c.wave {
        Some((kind, vals)) => Some(wave::read(lw, st, *kind, vals, c, &mut notes)?),
        None => {
            if c.td.is_some() || c.r.is_some() {
                notes.push(IGNORED);
            }
            None
        }
    };
    let mut dc = opt(c.dc, 0)?;
    if dc.is_none() && wave.is_none() {
        notes.push(NO_VALUE);
        dc = Some(int(0));
    }
    let ac = match (opt(c.ac_mag, 1)?, opt(c.ac_phase, 0)?) {
        (None, None) => None,
        (m, p) => Some((m.unwrap_or(int(1)), p.unwrap_or(int(0)))),
    };
    let mut distof = [None, None];
    for (k, d) in c.distof.iter().enumerate() {
        if let Some(vals) = d {
            let mag = val(lw, st, vals[0])?;
            let phase = vals.get(1).map_or(Ok(int(0)), |t| val(lw, st, t))?;
            distof[k] = Some((mag, phase));
        }
    }
    let port = match c.portnum {
        Some(n) => {
            let n = opt(Some(n), 0)?.expect("given");
            let z0 = opt(c.z0, 0)?.unwrap_or(int(50));
            if !positive_integer(&n) || matches!(z0, Val::Lit(l, _) if l.value() <= 0.0) {
                return Err(unread(PORT_NUMBER));
            }
            Some((n, z0))
        }
        None => {
            if c.z0.is_some() {
                notes.push(IGNORED);
            }
            None
        }
    };
    Ok(Read {
        dc,
        ac,
        distof,
        mult: opt(c.m, 0)?,
        port,
        wave,
        notes,
    })
}

/// Pushes the element and its rows.
fn push(lw: &mut Lower<'_, '_>, st: Stmt, head: &str, nodes: &[&str; 2], r: Read) {
    // rows in the order the IR text reads them back (name, terminals, `mult`, then the
    // detail's fields in declaration order), so that a read and its IR text are one design
    let name = lw.names.fold(head);
    let name = lw.b.name(&name);
    let terminals: Vec<String> = nodes.iter().map(|n| lw.names.node(n)).collect();
    let terminals: Vec<&str> = terminals.iter().map(String::as_str).collect();
    let terminals = lw.b.names(&terminals);
    let mult = emit_opt(lw, st, r.mult.as_ref());
    let dc_rule = if r.dc.is_none() {
        ir::source::DcRule::WaveAtZero
    } else {
        ir::source::DcRule::Given
    };
    let dc = emit_opt(lw, st, r.dc.as_ref());
    let (ac_mag, ac_phase_deg) = pair(lw, st, r.ac.as_ref());
    let wave = match r.wave.as_ref() {
        Some(w) => wave::push(lw, st, w),
        None => ir::WaveRef::NONE,
    };
    let port = match r.port.as_ref() {
        Some((n, z0)) => {
            let index = emit(lw, st, n);
            let z0 = emit(lw, st, z0);
            lw.b.detail(ir::source::PortSpec {
                index,
                z0,
                noise_temp: ir::ExprId::NONE,
                power: ir::ExprId::NONE,
                freq: ir::ExprId::NONE,
            })
        }
        None => ir::NO_ROW,
    };
    let distof = if r.distof.iter().any(Option::is_some) {
        let (f1_mag, f1_phase_deg) = pair(lw, st, r.distof[0].as_ref());
        let (f2_mag, f2_phase_deg) = pair(lw, st, r.distof[1].as_ref());
        lw.b.detail(ir::source::Distof {
            f1_mag,
            f1_phase_deg,
            f2_mag,
            f2_phase_deg,
        })
    } else {
        ir::NO_ROW
    };
    let kind = match (port != ir::NO_ROW, head.as_bytes()[0].to_ascii_lowercase()) {
        (true, _) => ir::ElemKind::Port,
        (false, b'i') => ir::ElemKind::ISource,
        (false, _) => ir::ElemKind::VSource,
    };
    let head = ir::ElemHead {
        name,
        terminals,
        model: ir::Ref::unresolved(ir::NameId::NONE),
        params: ir::List::EMPTY,
        mult,
        mult_law: ir::MultLaw::Parallel,
    };
    let row = ir::source::Source {
        dc,
        ac_mag,
        ac_phase_deg,
        tc1: ir::ExprId::NONE,
        tc2: ir::ExprId::NONE,
        pac_mag: ir::ExprId::NONE,
        pac_phase_deg: ir::ExprId::NONE,
        wave,
        noise_psd: ir::List::EMPTY,
        parasitics: ir::NO_ROW,
        port,
        distof,
        dc_rule,
    };
    let span = lw.span(st.node);
    lw.b.source(st.scope, kind, head, row, span);
}

/// Appends a (magnitude, phase) pair, or two `NONE`s.
fn pair(lw: &mut Lower<'_, '_>, st: Stmt, p: Option<&(Val, Val)>) -> (ir::ExprId, ir::ExprId) {
    match p {
        Some((a, b)) => (emit(lw, st, a), emit(lw, st, b)),
        None => (ir::ExprId::NONE, ir::ExprId::NONE),
    }
}

/// Whether a value is a literal whole number of at least 1 (`portnum` is an integer
/// parameter; ngspice rounds a fraction, which the IR's field would not).
fn positive_integer(v: &Val) -> bool {
    matches!(v, Val::Lit(l, _) if l.value() >= 1.0 && l.value().fract() == 0.0)
}

/// `tok` without the `{…}` or `'…'` around it.
fn unwrap(tok: &str) -> &str {
    let wrapped = |o: char, c: char| tok.len() >= 2 && tok.starts_with(o) && tok.ends_with(c);
    if wrapped('{', '}') || wrapped('\'', '\'') {
        &tok[1..tok.len() - 1]
    } else {
        tok
    }
}

/// A note row.
const fn row(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
) -> Row {
    (
        code,
        ir::Info {
            title,
            cite,
            explain,
            kind: ir::Kind::Note,
        },
    )
}
