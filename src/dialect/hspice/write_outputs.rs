//! `Save`, `Print`, `Measure` items → `.save`, `.print`, `.meas` cards, spelled as
//! `read_outputs.rs` reads them (law L5): a probe is `v(…)`, `vm vp vr vdb(…)`, `v(a,b)`,
//! `i(e)` or `@e[q]`, a path dotted as ngspice flattens it (`x1.n`, `v.x1.v9`); a measure in
//! the form of its kind, its values bare or in `{…}` (numparam), a `param=` in quotes.
//!
//! `admits` (R21) refuses an item whose spelling would read back as another one: a probe is
//! spelled and read back by the reader's own word rule (`read_outputs::probe_word`) in the
//! card it stands in, so `.save v(a,b)` (two saves) or an ac measure of a complex value (ngspice
//! reads the real part) never pass; the measure rules are the reader's rules, one line each.
//!
//! Cites: ngspice-45 `frontend/dotcards.c`, `frontend/com_measure2.c`, `frontend/measure.c`;
//! `rewrite/DIALECT-HSPICE.md` §4; `rewrite/RULINGS.md` R21.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const UNSPELLABLE`,
//! `pub(super) const CODES: &[Row]`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn save`, `print`, `measure`,
//! `pub(super) fn probe(&ir::Design, &ir::Probe, &mut String)`, `pub(super) fn probe_admits(&ir::Design, &ir::Probe) -> bool`

use super::codes::Row;
use super::expr_write;
use super::read_outputs::{self, Ctx};
use super::write::Out;
use crate::ir::measure::{MeasureFind, MeasureParam, MeasureStat, MeasureTrigTarg, MeasureWhen};
use crate::ir::{
    self, AcPart, AnalysisKind, Code, Edge, Event, EventKind, ExprClass, MeasureKind, ProbeKind,
    Stat,
};

/// The rows this area spells: exactly the reader's image (L5).
pub(super) const FEATURES: ir::FeatureSet = read_outputs::FEATURES;

/// An output item ngspice has no card for that reads back as it.
pub(super) const UNSPELLABLE: Code = Code::new(b'W', 4350);

/// This area's codes.
pub(super) const CODES: &[Row] = &[(
    UNSPELLABLE,
    ir::Info {
        title: "an output ngspice cannot spell",
        cite: "ngspice-45 dotcards.c gettoks, fixem; com_measure2.c get_value, measure_parse_when",
        explain: "a save of a difference or a part, a print of another analysis, a measure of another analysis, of a difference, of the complex value of an ac result, a threshold expression of the circuit, a delay outside tran, a crossing as a window bound; refused.",
        kind: ir::Kind::Refusal,
    },
)];

/// Appends a refusal when `item` has no ngspice card that reads back as it (R21).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let why = why_not(design, item);
    if why != 0 {
        diags.push_at(UNSPELLABLE, design, item, why);
    }
}

/// A `Save`: `.save p …` for the whole run, a script's `save p …` from where it stands, in the
/// script's order.
pub(super) fn save(w: &mut Out<'_>, item: ir::ItemRef) {
    debug_assert!(spellable(w.design, item), "ngspice write: an admitted save");
    let s: ir::Save = w.design.row(item.index);
    let words = save_words(w.design, s.probes).expect("ngspice write: an admitted save");
    match s.holds {
        ir::Applies::WholeRun => w.line(&format!(".save {}", words.join(" "))),
        ir::Applies::FromHere => unreachable!("HSPICE write: FEATURES lacks a save from here"),
    }
}

/// A `Print`: `.print kind p …`.
pub(super) fn print(w: &mut Out<'_>, item: ir::ItemRef) {
    debug_assert!(
        spellable(w.design, item),
        "ngspice write: an admitted print"
    );
    let p: ir::Print = w.design.row(item.index);
    let mut t = format!(".print {}", analysis(p.analysis));
    probes(w.design, p.probes, Ctx::Print(p.analysis), &mut t);
    w.line(&t);
}

/// A `Measure`: `.meas kind name …` in the form of its kind.
pub(super) fn measure(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    debug_assert!(spellable(d, item), "ngspice write: an admitted measure");
    let m: ir::Measure = d.row(item.index);
    let ctx = Ctx::Meas(m.analysis);
    let mut t = format!(".meas {} {} ", analysis(m.analysis), d.str(m.name));
    match m.kind {
        MeasureKind::TrigTarg => {
            let r: MeasureTrigTarg = d.detail(m.detail);
            t.push_str("trig ");
            clause(d, &r.trig, ctx, &mut t);
            t.push_str(" targ ");
            clause(d, &r.targ, ctx, &mut t);
        }
        MeasureKind::When => {
            let r: MeasureWhen = d.detail(m.detail);
            t.push_str("when ");
            crossing(d, &r.at, ctx, &mut t);
        }
        MeasureKind::Find => {
            let r: MeasureFind = d.detail(m.detail);
            t.push_str("find ");
            t.push_str(&spelled(d, &r.find, ctx));
            if r.at.kind == EventKind::At {
                t.push_str(" at=");
                expr_write::value(d, r.at.value, &mut t);
            } else {
                t.push_str(" when ");
                crossing(d, &r.at, ctx, &mut t);
            }
        }
        MeasureKind::Stat => {
            let r: MeasureStat = d.detail(m.detail);
            t.push_str(stat(r.stat));
            t.push(' ');
            t.push_str(&spelled(d, &r.signal, ctx));
            for (key, e) in [(" from=", &r.from), (" to=", &r.to)] {
                if e.kind == EventKind::At {
                    t.push_str(key);
                    expr_write::value(d, e.value, &mut t);
                }
            }
        }
        MeasureKind::Param => {
            let r: MeasureParam = d.detail(m.detail);
            t.push_str("param='");
            expr_write::expr(d, r.value, &mut t);
            t.push('\'');
        }
        MeasureKind::Deriv => unreachable!("ngspice write: FEATURES lacks MeasDeriv"),
    }
    w.line(&t);
}

/// Appends one output named inside another card (`.four`, `.tf`, `.noise`) as ngspice spells
/// it there: `v(a)`, `v(a,b)`, `i(e)`. Asserts [`probe_admits`] (the run area refuses first).
#[allow(dead_code, reason = "the run area calls it (S3.3a)")]
pub(super) fn probe(design: &ir::Design, probe: &ir::Probe, out: &mut String) {
    assert!(
        probe_admits(design, probe),
        "ngspice write: an admitted analysis output"
    );
    out.push_str(&spelled(design, probe, Ctx::Output));
}

/// Whether an analysis output has an ngspice spelling that reads back as it (the run area's
/// `admits` calls it for `.four`, `.tf`, `.noise` outputs).
#[allow(dead_code, reason = "the run area calls it (S3.3a)")]
pub(super) fn probe_admits(design: &ir::Design, probe: &ir::Probe) -> bool {
    reads_back(design, probe, Ctx::Output)
}

// ---- private ----

/// A probe as ngspice spells it in `ctx`, or `None` for a kind or part it has no word for. A
/// measure's real part is `v(…)`: its `vr(…)` saves nothing in a batch run (`gettok_iv` takes
/// `vr` for the vector; run: "can't parse 'vr'", the analysis does not run).
fn spell(design: &ir::Design, p: &ir::Probe, ctx: Ctx) -> Option<String> {
    let path =
        |l: ir::NameList| -> Vec<&str> { design.names(l).iter().map(|&n| design.str(n)).collect() };
    let a = path(p.a);
    Some(match p.kind {
        ProbeKind::V | ProbeKind::Vdiff => {
            let head = match p.part {
                AcPart::None => "v",
                AcPart::Re if matches!(ctx, Ctx::Meas(_)) => "v",
                AcPart::Re => "vr",
                AcPart::Mag => "vm",
                AcPart::PhaseRad => "vp",
                AcPart::Db => "vdb",
                AcPart::Im | AcPart::PhaseDeg => return None,
            };
            if p.kind == ProbeKind::Vdiff {
                format!("{head}({},{})", a.join("."), path(p.b).join("."))
            } else {
                format!("{head}({})", a.join("."))
            }
        }
        ProbeKind::I => format!("i({})", element(&a)?),
        ProbeKind::Param => {
            // only a name the device has a vector for (F13: `@i1[i]` is empty, `write` fails)
            let (dev, name) = (element(&a)?, *path(p.b).first()?);
            let letter = dev.rsplit('.').next()?.bytes().next()?.to_ascii_lowercase();
            if !super::outputs::readable(letter, name) {
                return None;
            }
            format!("@{dev}[{name}]")
        }
        _ => return None,
    })
}

/// Whether every part of `item` has a card that reads back as it.
fn spellable(d: &ir::Design, item: ir::ItemRef) -> bool {
    why_not(d, item) == 0
}

/// Why `item` has no card, the refusal's `arg` (W4350; 0: it has one): 1 retired (a save of
/// some analyses: the features refuse it), 2 a save probe with no word, 3 a print of another analysis, 4 a print view
/// other than the main one, 5 a print probe with no word, then a measure's: 6 its analysis,
/// 7 its name, `8 + MeasureKind as u32` its body (trig/targ, when, find, stat, param, deriv).
fn why_not(d: &ir::Design, item: ir::ItemRef) -> u32 {
    let first = |checks: &[(bool, u32)]| checks.iter().find(|c| !c.0).map_or(0, |c| c.1);
    match item.kind {
        ir::ItemKind::Save => {
            // a save of some analyses is the features' (`SavePerAnalysis`, `SaveAnalysisSet`:
            // FEATURES has neither), not a refusal here (R21)
            let s: ir::Save = d.row(item.index);
            first(&[(save_words(d, s.probes).is_some(), 2)])
        }
        ir::ItemKind::Print => {
            let p: ir::Print = d.row(item.index);
            let kind = matches!(
                p.analysis,
                AnalysisKind::Tran | AnalysisKind::Ac | AnalysisKind::Dc
            );
            let ctx = Ctx::Print(p.analysis);
            first(&[
                (kind, 3),
                (p.view == ir::PrintView::Main, 4),
                (d.probes(p.probes).iter().all(|q| reads_back(d, q, ctx)), 5),
            ])
        }
        ir::ItemKind::Measure => measure_why(d, d.row(item.index)),
        _ => unreachable!("write_outputs::admits: an output item"),
    }
}

/// The measure rules of `read_outputs/meas.rs`, as checks: the first that fails (`why_not`'s
/// numbers), or 0.
fn measure_why(d: &ir::Design, m: ir::Measure) -> u32 {
    let kind = matches!(
        m.analysis,
        AnalysisKind::Tran | AnalysisKind::Ac | AnalysisKind::Dc | AnalysisKind::Sp
    );
    let name = d.str(m.name);
    let name_ok =
        !name.is_empty() && !name.contains(|c: char| c.is_whitespace() || "=('{\"".contains(c));
    let ctx = Ctx::Meas(m.analysis);
    let dc = m.analysis == AnalysisKind::Dc;
    let complex = matches!(m.analysis, AnalysisKind::Ac | AnalysisKind::Sp);
    let signal = |p: &ir::Probe| p.kind != ProbeKind::Vdiff && reads_back(d, p, ctx);
    let value = |e: ir::ExprId| constant(d, e);
    // a `when` threshold may be one `v(x)` or `i(x)`, a trigger's `val=` is a number; only a
    // `when` takes ngspice's default pick (no edge word: any edge); dc has no typed window
    let cross = |e: &Event, when: bool| {
        e.kind == EventKind::Cross
            && signal(&e.signal)
            && (value(e.value) || (when && !complex && threshold(d, e.value).is_some()))
            && (e.count.is_none() || value(e.count))
            && (!e.skip_initial || (when && e.edge == Edge::Cross))
            && (e.delay.is_none() || (!dc && value(e.delay)))
            && (e.until.is_none() || (!dc && value(e.until) && !zero(d, e.until)))
    };
    // ac and sp spell a delay `from=`, which ngspice copies to the other clause when it has none
    // and it is not 0 (get_measure2); a trigger's `from=` must be a number
    let copied = |e: &Event| !e.delay.is_none() && !zero(d, e.delay);
    let literal = |e: &Event| e.delay.is_none() || matches!(d.expr(e.delay), ir::Expr::Num(_));
    let pair = |a: &Event, b: &Event| {
        !complex
            || (literal(a)
                && literal(b)
                && (copied(a) == copied(b) || !a.delay.is_none() && !b.delay.is_none()))
    };
    let at = |e: &Event| e.kind == EventKind::At && value(e.value);
    let bound = |e: &Event, to: bool| {
        e.kind == EventKind::Bound || (at(e) && !(to && !dc && zero(d, e.value)))
    };
    // dc swaps a window given in the other order (measure_parse_stdParams): only numbers, in order
    let ordered = |a: &Event, b: &Event| {
        let num = |e: &Event| match d.expr(e.value) {
            ir::Expr::Num(n) => Some(d.literal_value(d.num(n))),
            _ => None,
        };
        !dc || a.kind != EventKind::At
            || b.kind != EventKind::At
            || matches!((num(a), num(b)), (Some(x), Some(y)) if x <= y)
    };
    let body = match m.kind {
        MeasureKind::TrigTarg => {
            let r: MeasureTrigTarg = d.detail(m.detail);
            (cross(&r.trig, false) || at(&r.trig))
                && (cross(&r.targ, false) || at(&r.targ))
                && pair(&r.trig, &r.targ)
        }
        MeasureKind::When => cross(&d.detail::<MeasureWhen>(m.detail).at, true),
        MeasureKind::Find => {
            let r: MeasureFind = d.detail(m.detail);
            signal(&r.find) && (cross(&r.at, true) || at(&r.at))
        }
        MeasureKind::Stat => {
            let r: MeasureStat = d.detail(m.detail);
            signal(&r.signal)
                && bound(&r.from, false)
                && bound(&r.to, true)
                && ordered(&r.from, &r.to)
        }
        MeasureKind::Param => constant(d, d.detail::<MeasureParam>(m.detail).value),
        MeasureKind::Deriv => false,
    };
    let checks = [(kind, 6), (name_ok, 7), (body, 8 + m.kind as u32)];
    checks.iter().find(|c| !c.0).map_or(0, |c| c.1)
}

/// Whether `p` spelled in `ctx` reads back as one probe of its kind, part and paths.
fn reads_back(d: &ir::Design, p: &ir::Probe, ctx: Ctx) -> bool {
    let Some(text) = spell(d, p, ctx) else {
        return false;
    };
    if read_outputs::words(&text) != [text.clone()] {
        return false;
    }
    let names = |l: ir::NameList| -> Vec<String> {
        d.names(l).iter().map(|&n| d.str(n).to_string()).collect()
    };
    match read_outputs::probe_word(&text, ctx).as_deref() {
        Ok([w]) => w.kind == p.kind && w.part == p.part && w.a == names(p.a) && w.b == names(p.b),
        _ => false,
    }
}

/// Whether `e` is a constant value HSPICE spells.
fn constant(d: &ir::Design, e: ir::ExprId) -> bool {
    !e.is_none() && ir::expr_class(d, e) == ExprClass::Constant && expr_write::spells(d, e)
}

/// A threshold that reads the circuit as `v(n)` or `i(e)` (one name, no dots), if it is one.
fn threshold(d: &ir::Design, e: ir::ExprId) -> Option<String> {
    let word = |n: ir::NameId| {
        let s = d.str(n);
        let plain =
            !s.is_empty() && !s.contains(|c: char| c.is_whitespace() || "().,=".contains(c));
        plain.then(|| s.to_string())
    };
    match d.expr(e) {
        ir::Expr::V(n, r) if r.is_none() => Some(format!("v({})", word(n)?)),
        ir::Expr::I(n) => Some(format!("i({})", word(n)?)),
        _ => None,
    }
}

/// Whether `e` is the literal zero (`to=0` is "no limit" to ngspice).
fn zero(d: &ir::Design, e: ir::ExprId) -> bool {
    matches!(d.expr(e), ir::Expr::Num(n) if d.literal_eq(d.num(n), ir::Literal::ZERO))
}

/// ` p …` for each probe of the list.
fn probes(d: &ir::Design, list: ir::ProbeList, ctx: Ctx, t: &mut String) {
    for p in d.probes(list) {
        t.push(' ');
        t.push_str(&spelled(d, p, ctx));
    }
}

/// A probe the writer was handed (admitted).
fn spelled(d: &ir::Design, p: &ir::Probe, ctx: Ctx) -> String {
    spell(d, p, ctx).expect("ngspice write: an admitted probe")
}

/// A trigger or target clause: `at=v` or `sig val=v [td=d] edge=n|last`.
fn clause(d: &ir::Design, e: &Event, ctx: Ctx, t: &mut String) {
    if e.kind == EventKind::At {
        t.push_str("at=");
        expr_write::value(d, e.value, t);
        return;
    }
    t.push_str(&spelled(d, &e.signal, ctx));
    t.push_str(" val=");
    expr_write::value(d, e.value, t);
    tail(d, e, ctx, t);
}

/// A `when` clause: `sig=v [td=d] edge=n|last`.
fn crossing(d: &ir::Design, e: &Event, ctx: Ctx, t: &mut String) {
    t.push_str(&spelled(d, &e.signal, ctx));
    t.push('=');
    match threshold(d, e.value).filter(|_| !constant(d, e.value)) {
        Some(probe) => t.push_str(&probe),
        None => expr_write::value(d, e.value, t),
    }
    tail(d, e, ctx, t);
}

/// ` [td=d|from=d] [to=u] [edge=n|last]`: tran spells the delay `td=`, ac and sp `from=`
/// (ngspice reads TD in tran only); ngspice's default pick has no edge word.
fn tail(d: &ir::Design, e: &Event, ctx: Ctx, t: &mut String) {
    let td = if ctx == Ctx::Meas(AnalysisKind::Tran) {
        " td="
    } else {
        " from="
    };
    for (key, x) in [(td, e.delay), (" to=", e.until)] {
        if !x.is_none() {
            t.push_str(key);
            expr_write::value(d, x, t);
        }
    }
    if e.skip_initial {
        return;
    }
    t.push(' ');
    t.push_str(match e.edge {
        Edge::Rise => "rise=",
        Edge::Fall => "fall=",
        Edge::Cross => "cross=",
    });
    if e.last {
        t.push_str("last");
    } else {
        expr_write::value(d, e.count, t);
    }
}

/// The words of a save list: `all` for `AllV` then `AllBranchI` (ngspice's `all`), each other
/// probe its own word that reads back as it; `None` when one has none.
fn save_words(d: &ir::Design, list: ir::ProbeList) -> Option<Vec<String>> {
    let probes = d.probes(list);
    let mut out = Vec::new();
    let mut k = 0;
    // Bounded: each step takes one or two probes.
    while let Some(p) = probes.get(k) {
        let next = probes.get(k + 1).map(|q| q.kind);
        let plain =
            p.part == AcPart::None && probes.get(k + 1).is_some_and(|q| q.part == AcPart::None);
        if p.kind == ProbeKind::AllV && next == Some(ProbeKind::AllBranchI) && plain {
            out.push("all".to_string());
            k += 2;
            continue;
        }
        if !reads_back(d, p, Ctx::Save) {
            return None;
        }
        out.push(spelled(d, p, Ctx::Save));
        k += 1;
    }
    Some(out)
}

/// An element path as ngspice flattens it: `name`, or `letter.x1.….name`.
fn element(a: &[&str]) -> Option<String> {
    let (last, inner) = a.split_last()?;
    if inner.is_empty() {
        return Some(last.to_string());
    }
    let letter = last.chars().next()?;
    Some(format!("{letter}.{}.{last}", inner.join(".")))
}

/// The analysis word of a print or a measure.
fn analysis(kind: AnalysisKind) -> &'static str {
    match kind {
        AnalysisKind::Tran => "tran",
        AnalysisKind::Ac => "ac",
        AnalysisKind::Dc => "dc",
        AnalysisKind::Sp => "sp",
        other => unreachable!("ngspice write: an admitted analysis kind, not {other:?}"),
    }
}

/// The function word of a statistic.
fn stat(s: Stat) -> &'static str {
    match s {
        Stat::Avg => "avg",
        Stat::Min => "min",
        Stat::Max => "max",
        Stat::Pp => "pp",
        Stat::Rms => "rms",
        Stat::Integ => "integ",
        Stat::MinAt => "min_at",
        Stat::MaxAt => "max_at",
    }
}
