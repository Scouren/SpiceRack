//! Instances of VACASK's builtin sources and couplings → `Element` rows: `vsource`/`isource`
//! (`type=` dc sine pulse exp pwl am fm, `mag`, `phase`) with their `Source` row and wave,
//! `vcvs`/`vccs`/`cccs`/`ccvs` with `Controlled`, `mutual` with `Coupling`; every field VACASK
//! reads is filled with VACASK's default (`DEFAULTS`, run src-dump), so the row states what the
//! binary simulates.
//!
//! Rules (each with its run, `tests/fixtures/dialect/vacask/settle/src_*.sim`):
//! - a wave `type` ignores `dc`: every analysis reads the wave, the operating point its value at
//!   t = 0 (`DcRule::WaveAtZero`; src-sine, src-pulse, src-exp); `type="dc"` is `Given`;
//! - no `mag` and no `phase`: no AC stimulus; either: both, the other at its default 0;
//! - a parameter the `type` does not read is kept by VACASK for a later `alter … type=` (VS V5):
//!   with no such alter it moves no number, dropped with a note (R42);
//! - what VACASK rejects is `InvalidInSource` (src-r-*): an unknown parameter, a terminal count
//!   the module does not have, a literal `rise` ≤ 0 (p2), no `td2` or a literal one ≤ 0, no `tau1`/`tau2` or a zero one (NaN), a
//!   literal sine `freq` ≤ 0, a PWL with an odd count or one point, a missing `ctlinst`,
//!   `ind1`, `ind2`, `$mfactor` on a `mutual`;
//! - what only VACASK has is `Private`: PWL shaping and breakpoints off their defaults, the
//!   spurs, a `ctlnode` other than the branch flow, a value the IR cannot hold (a `type` or a
//!   name that is no string literal, a `wave` that is no vector literal).
//!
//! `tline_ideal` (R39, A20; run t3) is a `LosslessLine` naming its `TlineIdeal` model: `td`
//! given is the delay (`f`, `nl` beside it are ignored by VACASK: dropped with `LINE_IGNORED`);
//! else `f` and `nl`, each at VACASK's default (1k, 1) when absent; `z0` 50 by default.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (sources … mutual); VD dev-builtin-src,
//! dev-builtin-vcvs, -vccs, -cccs, -ccvs, -mutual, cir-mfactor; R39, A20.
//!
//! Contents: `FEATURES`, `INVALID`, `SHAPING`, `DORMANT`, `SPURS`, `CTLNODE`, `LINE_IGNORED`,
//! `TYPE_ALTER`, `UNSTATED`, `CODES`, `DEFAULTS`, `default_of`, `source` (crate-private); the statement's
//! parts in `common.rs`, the independent sources in `wave.rs`

mod common;
mod wave;

use super::read::{self, Lower};
use super::read_elements::{self, Found, Master};
use super::read_structure;
use super::tables::SOURCE_MODULES;
use crate::ir;
use crate::ir::CircuitExtra as X;
use crate::ir::Feature as F;
use crate::syntax::ast::Node;
use common::{
    Fail, Params, Val, build, check, flow_node, head, invalid, mult, params, string, terminals,
};

/// What this area's rows use (its writer spells the same set, law L5).
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(F::Item(ir::ItemKind::Element))
    .with(F::Elem(ir::ElemKind::VSource))
    .with(F::Elem(ir::ElemKind::ISource))
    .with(F::Elem(ir::ElemKind::Vcvs))
    .with(F::Elem(ir::ElemKind::Vccs))
    .with(F::Elem(ir::ElemKind::Cccs))
    .with(F::Elem(ir::ElemKind::Ccvs))
    .with(F::Elem(ir::ElemKind::Coupling))
    .with(F::Elem(ir::ElemKind::LosslessLine))
    .with(F::Circuit(X::LineFreqNl))
    .with(F::Circuit(X::MultOther))
    .with(F::Wave(ir::WaveKind::Pulse))
    .with(F::Wave(ir::WaveKind::Sin))
    .with(F::Wave(ir::WaveKind::Exp))
    .with(F::Wave(ir::WaveKind::Pwl))
    .with(F::Wave(ir::WaveKind::Sffm))
    .with(F::Wave(ir::WaveKind::Am))
    .with(F::Circuit(X::BuiltinMaster))
    .with(F::Circuit(X::NameWithoutLetter))
    .with(F::Circuit(X::GivenZeroAsGiven))
    .with(F::Circuit(X::MultVSource))
    .with(F::Circuit(X::MultISource))
    .with(F::Circuit(X::MultControlled))
    .with(F::Circuit(X::PulseVacask))
    .with(F::Circuit(X::SinPreHold))
    .with(F::Circuit(X::ExpTd2FromTd1))
    .with(F::Circuit(X::SffmSpice3))
    .with(F::Circuit(X::SffmPreHold))
    .with(F::Circuit(X::AmPreHold))
    .with(F::Circuit(X::PwlDelay))
    .with(F::Circuit(X::PwlRepeatPeriod))
    .with(F::Circuit(X::PwlJoinExtrapolate))
    .with(F::Circuit(X::PwlFile))
    .with(F::Circuit(X::DcAtZeroPulse))
    .with(F::Circuit(X::DcAtZeroSin))
    .with(F::Circuit(X::DcAtZeroExp))
    .with(F::Circuit(X::DcAtZeroPwl))
    .with(F::Circuit(X::DcAtZeroSffm))
    .with(F::Circuit(X::DcAtZeroAm))
    .with(F::Circuit(X::DcGivenWithWave));

/// A source statement VACASK rejects.
pub(super) const INVALID: ir::Code = ir::Code::new(b'P', 2201);
/// PWL shaping or breakpoints off their defaults.
pub(super) const SHAPING: ir::Code = ir::Code::new(b'P', 2202);
/// A parameter the source's `type` does not read, off its default.
pub(super) const DORMANT: ir::Code = ir::Code::new(b'P', 2203);
/// `spur`, `smag`, `sphase`.
pub(super) const SPURS: ir::Code = ir::Code::new(b'P', 2204);
/// A `ctlnode` other than the branch flow.
pub(super) const CTLNODE: ir::Code = ir::Code::new(b'P', 2205);
/// `tline_ideal` given both `td` and `f`/`nl`: VACASK reads `td` (R39).
pub(super) const LINE_IGNORED: ir::Code = ir::Code::new(b'P', 2206);
/// A source a `type` alter reaches other than by R42's alternation.
pub(super) const TYPE_ALTER: ir::Code = ir::Code::new(b'P', 2208);
/// A value VACASK reads that the IR cannot hold here.
pub(super) const UNSTATED: ir::Code = ir::Code::new(b'P', 2207);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    row(
        INVALID,
        "a source statement VACASK rejects",
        "VD dev-builtin-src, -cccs, -mutual; runs p2, src-r-*",
        "An unknown parameter, a terminal count the module does not have, a literal rise at or below 0, no td2 or one at or below 0, an exp time constant absent or 0 (the run fails with NaN), a sine frequency at or below 0, a PWL with an odd count or one point, a cccs/ccvs without ctlinst, a mutual without ind1 or ind2, with terminals or with $mfactor. The statement is kept as text and the deck refused.",
        ir::Kind::Refusal,
    ),
    row(
        SHAPING,
        "PWL shaping or breakpoints VACASK alone has",
        "DIALECT-VACASK §3.2 pwl, §5 G5; VD dev-builtin-src (PWL)",
        "scale, offset, stretch or tdphase off 1, 0, 1, 0, break other than \"all\" or a slope tolerance off its default: no IR field states them yet. The statement is kept as text.",
        ir::Kind::Note,
    ),
    row(
        DORMANT,
        "a source parameter its type does not read, dropped",
        "R42; VD dev-builtin-src",
        "VACASK keeps it for a later `alter … type=`; with no such alter it moves no number, so it is dropped (a `wave` beside a `file` too).",
        ir::Kind::Note,
    ),
    row(
        SPURS,
        "the (quasi)periodic small-signal spurs",
        "VD dev-builtin-src (spur, smag, sphase)",
        "hbac and pac excitation lists have no IR field. The statement is kept as text.",
        ir::Kind::Note,
    ),
    row(
        CTLNODE,
        "a controlling node other than the branch flow",
        "DIALECT-VACASK §3.2 controlled; VD dev-builtin-cccs, -mutual",
        "ctlnode, ctlnode1, ctlnode2 other than \"flow(br)\" read another internal unknown of the instance. The statement is kept as text.",
        ir::Kind::Note,
    ),
    row(
        LINE_IGNORED,
        "an ideal line given td and f: f and nl ignored",
        "R39; run t3 (td=2n f=100M arrives at 2 ns)",
        "VACASK reads the delay td when it is given; f and nl are dropped.",
        ir::Kind::Note,
    ),
    row(
        super::write_sources::AM_OFFSET,
        "an AM wave with a modulation offset other than 1",
        "VD dev-builtin-src (AM: 1 + modindex·sin(…))",
        "VACASK's AM fixes the offset inside the modulating factor at 1; no parameter spells another.",
        ir::Kind::Refusal,
    ),
    row(
        super::write_sources::PWL_JUMP,
        "a periodic PWL that jumps back to its first point",
        "VD dev-builtin-src (PWL period); A22 (PwlJoin::Extrapolate is VACASK's law)",
        "VACASK's periodic PWL joins its last point to the next period's first by a line; a jump has no spelling.",
        ir::Kind::Refusal,
    ),
    row(
        super::write_sources::PARAMS,
        "a source, controlled source or mutual with instance parameters",
        "VD dev-builtin-src, -vcvs, -cccs, -mutual; S 7.11",
        "VACASK's builtins take only their own parameters; any other is \"Parameter not found\".",
        ir::Kind::Refusal,
    ),
    row(
        TYPE_ALTER,
        "a source whose type an alter changes otherwise than the dc/wave alternation",
        "R42; VD cmd-alter",
        "Only the alternation of `type=\"dc\"` before DC-kind analyses and one wave before transients states a source row (DcRule::Given with the wave); any other `type` alter makes the line and its alters VACASK's alone.",
        ir::Kind::Note,
    ),
    row(
        UNSTATED,
        "a source value the IR cannot hold",
        "VD dev-builtin-src; DIALECT-VACASK §3.2 sources",
        "a type, ctlinst, ind1, ind2 or file that is no string literal, a wave that is no vector literal, or a period that is no literal. The statement is kept as text.",
        ir::Kind::Note,
    ),
];

/// VACASK's default of every source parameter (run src-dump: `print instance` of a bare
/// `vsource`, a `cccs` and a `mutual`), as the literal the reader fills; a string's quoted.
pub(super) const DEFAULTS: &[(&str, &str)] = &[
    ("dc", "0."),
    ("mag", "0."),
    ("phase", "0."),
    ("delay", "0."),
    ("val0", "0."),
    ("val1", "1."),
    ("rise", "1n"),
    ("fall", "0."),
    ("width", "0."),
    ("period", "0."),
    ("sinedc", "0."),
    ("ampl", "1."),
    ("freq", "1k"),
    ("tdphase", "0."),
    ("theta", "0."),
    ("td2", "0."),
    ("tau1", "0."),
    ("tau2", "0."),
    ("modfreq", "1k"),
    ("modphase", "0."),
    ("modindex", "0.5"),
    ("offset", "0."),
    ("scale", "1."),
    ("stretch", "1."),
    ("break", "\"all\""),
    ("slopetol", "0."),
    ("sloperel", "0."),
    ("slopeglob", "0.01"),
    ("file", "\"\""),
    ("gain", "1."),
    ("z0", "50."),
    ("f", "1k"),
    ("nl", "1."),
    ("k", "0."),
    ("ctlnode", "\"flow(br)\""),
    ("ctlnode1", "\"flow(br)\""),
    ("ctlnode2", "\"flow(br)\""),
];

/// VACASK's default of parameter `key` (`DEFAULTS`), or `None`.
pub(super) fn default_of(key: &str) -> Option<&'static str> {
    DEFAULTS.iter().find(|(k, _)| *k == key).map(|&(_, v)| v)
}

/// An instance of a model of a builtin source or coupling module (the spine routes it here,
/// `read::instance_area`), as one `Element` or one `Opaque`.
pub(super) fn source(l: &mut Lower<'_, '_, '_>, n: Node) {
    let h = l.cst.ast.header(n);
    let master = read::field_name(l.cst, h.master);
    // The master as VACASK binds it (vc-devices' declared table: own subckt, then the top).
    let module = match read_elements::find(l, l.declared, &master) {
        Found::Here(Master::Model(word)) if SOURCE_MODULES.contains(&word.as_str()) => Some(word),
        Found::Enclosing => {
            return read::opaque(
                l,
                n,
                ir::OpaqueClass::InvalidInSource,
                Some(read_structure::ENCLOSING),
            );
        }
        // A name a source model has elsewhere, but not as seen from here: the devices area's.
        _ => return read_elements::instance(l, n),
    };
    let result = params(l, n).and_then(|p| match module.as_deref() {
        Some("vsource" | "isource") => {
            wave::independent(l, n, &p, module.as_deref() == Some("isource"))
        }
        Some(m @ ("vcvs" | "vccs" | "cccs" | "ccvs")) => controlled(l, n, &p, m),
        Some("mutual") => mutual(l, n, &p),
        Some("tline_ideal") => line(l, n, &p),
        _ => unreachable!("routed here only for a model of a source module"),
    });
    if let Err((class, code)) = result {
        read::opaque(l, n, class, Some(code));
    }
}

// ---- private ----

/// One catalogue row.
const fn row(
    code: ir::Code,
    title: &'static str,
    cite: &'static str,
    explain: &'static str,
    kind: ir::Kind,
) -> (ir::Code, ir::Info) {
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

/// `vcvs`, `vccs` (`p n cp cn`, `gain`) and `cccs`, `ccvs` (`p n`, `ctlinst`, `gain`).
fn controlled(l: &mut Lower<'_, '_, '_>, n: Node, p: &Params, module: &str) -> Result<(), Fail> {
    let current = matches!(module, "cccs" | "ccvs");
    let (keys, count): (&[&str], usize) = match current {
        true => (&["gain", "ctlinst", "ctlnode", "$mfactor"], 2),
        false => (&["gain", "$mfactor"], 4),
    };
    p.only(keys)?;
    terminals(l, n, count)?;
    let sense = match current {
        true => Some(string(l, p.get("ctlinst").ok_or_else(invalid)?)?),
        false => None,
    };
    flow_node(l, p, "ctlnode")?;
    let gain = p.val("gain");
    let m = p.get("$mfactor").map(Val::Node);
    check(l, &[gain, m.unwrap_or(Val::Lit("1."))])?;
    let mult = mult(l, p);
    let gain = build(l, gain);
    let sense = sense.map_or(ir::NameId::NONE, |s| l.b.name(&s));
    let kind = match module {
        "vcvs" => ir::ElemKind::Vcvs,
        "vccs" => ir::ElemKind::Vccs,
        "cccs" => ir::ElemKind::Cccs,
        _ => ir::ElemKind::Ccvs,
    };
    let head = head(l, n, mult);
    let span = l.span(n);
    let at = l.b.controlled(
        l.scope,
        kind,
        head,
        ir::controlled::Controlled { gain, sense },
        span,
    );
    l.bind_master_later(at);
    Ok(())
}

/// `mutual`: no terminals, `k`, `ind1`, `ind2` (VD dev-builtin-mutual).
fn mutual(l: &mut Lower<'_, '_, '_>, n: Node, p: &Params) -> Result<(), Fail> {
    p.only(&["k", "ind1", "ind2", "ctlnode1", "ctlnode2"])?;
    terminals(l, n, 0)?;
    let ind1 = string(l, p.get("ind1").ok_or_else(invalid)?)?;
    let ind2 = string(l, p.get("ind2").ok_or_else(invalid)?)?;
    if ind1.is_empty() || ind2.is_empty() || ind1 == ind2 {
        return Err(invalid());
    }
    flow_node(l, p, "ctlnode1")?;
    flow_node(l, p, "ctlnode2")?;
    let k = p.val("k");
    check(l, &[k])?;
    let k = build(l, k);
    let inductors = l.b.names(&[ind1.as_str(), ind2.as_str()]);
    let head = head(l, n, ir::ExprId::NONE);
    let span = l.span(n);
    let at =
        l.b.coupling(l.scope, head, ir::passive::Coupling { inductors, k }, span);
    l.bind_master_later(at);
    Ok(())
}

/// `tline_ideal` (`p n p2 n2`; R39, run t3).
fn line(l: &mut Lower<'_, '_, '_>, n: Node, p: &Params) -> Result<(), Fail> {
    p.only(&["z0", "td", "f", "nl", "$mfactor"])?;
    terminals(l, n, 4)?;
    let z0 = p.val("z0");
    let by_td = p.get("td").map(Val::Node);
    let ignored = by_td.is_some() && (p.get("f").is_some() || p.get("nl").is_some());
    let (freq, nl) = match by_td {
        Some(_) => (None, None),
        None => (Some(p.val("f")), Some(p.val("nl"))),
    };
    let m = p.get("$mfactor").map(Val::Node);
    let vals: Vec<Val> = [Some(z0), by_td, freq, nl, m]
        .into_iter()
        .flatten()
        .collect();
    check(l, &vals)?;
    let mult = mult(l, p);
    let mut opt = |v: Option<Val>| v.map_or(ir::ExprId::NONE, |v| build(l, v));
    let row = ir::line::Line {
        z0: opt(Some(z0)),
        td: opt(by_td),
        freq: opt(freq),
        nl: opt(nl),
        length: ir::ExprId::NONE,
        vel: ir::ExprId::NONE,
        lumps: ir::ExprId::NONE,
    };
    let head = head(l, n, mult);
    let span = l.span(n);
    let at =
        l.b.line(l.scope, ir::ElemKind::LosslessLine, head, row, span);
    l.bind_master_later(at);
    if ignored {
        l.diags.push(LINE_IGNORED, span, 0);
    }
    Ok(())
}
