//! `.meas`/`.measure <analysis> <name> <function> …` → one `Measure`. The forms are those of
//! ngspice-45 `com_measure2.c` (`measure_extract_variables`' header, `get_measure2`):
//! `trig … targ …`, `when sig=rhs`, `find sig when …`, `find sig at=v`,
//! `avg|min|max|pp|rms|integ sig [from=] [to=]`, and `param=` (`measure.c` `do_measure`, read by
//! numparam after every other measure: its names bind to measures first, A7).
//!
//! Defaults ngspice fills, made explicit: a crossing's `val` is 0 (the clause is zeroed by
//! `TMALLOC`); a `when` with no `rise`/`fall`/`cross` is the first crossing, `cross=1`; the last
//! of `rise`/`fall`/`cross` given wins (`measure_parse_stdParams`). A value is a number, or a
//! numparam expression (`inpcom.c` braces any other right-hand side); a `when` right-hand side
//! may be one `v(x)` or `i(x)` (a threshold that reads the circuit).
//!
//! Contents: `pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String]) -> Result<(), No>`

use super::probe::{Ctx, Want, build, probe_word};
use super::{No, unwrap, words};
use crate::dialect::ngspice::declare::Class;
use crate::dialect::ngspice::expr_read::{self, Tree};
use crate::dialect::ngspice::name;
use crate::dialect::ngspice::number;
use crate::dialect::ngspice::read::{Lower, Stmt};
use crate::dialect::ngspice::tables::Eval;
use crate::ir::measure::{MeasureFind, MeasureParam, MeasureStat, MeasureTrigTarg, MeasureWhen};
use crate::ir::{self, AnalysisKind, Edge, Event, ProbeKind, Stat};
use crate::syntax::ast::{Node, Tag};

/// A value read but not yet appended.
enum Val {
    /// A number and how its digits fit (`number::Fit`).
    Lit(ir::Literal, number::Fit),
    /// A numparam expression.
    Tree(Tree),
    /// A threshold that reads the circuit: `v(x)` or `i(x)`.
    Probe(Want),
}

/// A crossing read but not yet appended.
struct Crossing {
    signal: Want,
    value: Val,
    edge: Edge,
    last: bool,
    count: Option<Val>,
    skip_initial: bool,
    delay: Option<Val>,
    until: Option<Val>,
}

/// An event read but not yet appended.
enum Ev {
    /// A crossing.
    Cross(Box<Crossing>),
    /// A position on the sweep axis.
    At(Val),
    /// The analysis's start or end.
    Bound,
}

/// A measure body read but not yet appended.
enum Body {
    TrigTarg(Ev, Ev),
    When(Ev),
    Find(Want, Ev),
    Stat(Stat, Want, Ev, Ev),
    Param(Tree),
}

/// What a clause's `key=value` words set.
#[derive(Default)]
struct Params {
    edge: Option<(Edge, Option<String>)>,
    val: Option<String>,
    td: Option<String>,
    at: Option<String>,
    from: Option<String>,
    to: Option<String>,
}

/// One `.meas` card.
pub(super) fn card(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String]) -> Result<(), No> {
    let [an, name, f, rest @ ..] = words else {
        return Err(No::Invalid); // "Incomplete measurement statement … ignored"
    };
    let kind = match an.to_ascii_lowercase().as_str() {
        "tran" => AnalysisKind::Tran,
        "ac" => AnalysisKind::Ac,
        "dc" => AnalysisKind::Dc,
        "sp" => AnalysisKind::Sp,
        _ => return Err(No::Invalid), // do_measure: "unrecognized analysis type"
    };
    if name.contains(['=', '(', '\'', '{']) {
        return Err(No::Invalid);
    }
    let ctx = Ctx::Meas(kind);
    let low = f.to_ascii_lowercase();
    let body = if low.starts_with("param=") || low.starts_with("expr=") {
        if !rest.is_empty() {
            return Err(No::Untyped);
        }
        let text = unwrap(&f[f.find('=').expect("a key=value word") + 1..]);
        later_param(lw, st, text)?;
        Body::Param(numparam(lw, st, text, false)?)
    } else {
        match low.as_str() {
            "trig" => trig_targ(lw, st, rest, ctx)?,
            "when" => Body::When(when(lw, st, rest, ctx)?),
            "find" => find(lw, st, rest, ctx)?,
            "avg" | "min" | "max" | "pp" | "rms" | "integ" => stat(lw, st, &low, rest, ctx)?,
            "min_at" | "max_at" => stat(lw, st, &low, rest, ctx)?,
            "delay" | "targ" => return Err(No::Untyped),
            _ => return Err(No::Invalid), // deriv, err*: "not supported"; else "no such function"
        }
    };
    let name = lw.names.fold(name);
    let id = lw.b.design().find_name(&name);
    if !id.is_none()
        && !lw
            .b
            .design()
            .lookup(st.scope, ir::DefKind::Measure, id)
            .is_none()
    {
        return Err(No::Untyped); // a repeated name: which result later reads see is not typed
    }
    lw.b.name(&name); // the name first, as IR text interns it (one order of the name table)
    let span = lw.span(st.node);
    match body {
        Body::TrigTarg(a, b) => {
            let row = MeasureTrigTarg {
                trig: event(lw, st, a),
                targ: event(lw, st, b),
            };
            lw.b.measure(st.scope, &name, kind, row, span);
        }
        Body::When(e) => {
            let row = MeasureWhen {
                at: event(lw, st, e),
            };
            lw.b.measure(st.scope, &name, kind, row, span);
        }
        Body::Find(w, e) => {
            let find = build(lw, &w);
            let row = MeasureFind {
                find,
                at: event(lw, st, e),
            };
            lw.b.measure(st.scope, &name, kind, row, span);
        }
        Body::Stat(stat, w, from, to) => {
            let signal = build(lw, &w);
            let from = event(lw, st, from);
            let row = MeasureStat {
                stat,
                signal,
                from,
                to: event(lw, st, to),
            };
            lw.b.measure(st.scope, &name, kind, row, span);
        }
        Body::Param(tree) => {
            let mark = lw.pending();
            let value = lw.expr(st, &tree, st.scope);
            lw.measure_reads(mark);
            lw.b.measure(st.scope, &name, kind, MeasureParam { value }, span);
        }
    }
    Ok(())
}

// ---- private ----

/// `trig <clause> targ <clause>`: each clause a crossing or `at=`.
fn trig_targ(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String], ctx: Ctx) -> Result<Body, No> {
    let k = words
        .iter()
        .position(|w| w.eq_ignore_ascii_case("targ"))
        .ok_or(No::Invalid)?;
    let (mut a, mut b) = (head(&words[..k], ctx)?, head(&words[k + 1..], ctx)?);
    // a FROM of one clause is the other's when that one has none, unless it is 0 (get_measure2:
    // "If there was a FROM propagate trig<->targ", `m_from != 0.0`); an expression is not read
    let given = |f: &Option<String>| -> Result<Option<String>, No> {
        match f.as_deref().map(number::card) {
            None => Ok(None),
            Some(None) => Err(No::Untyped),
            Some(Some(n)) => Ok((n.literal.mantissa != 0).then(|| f.clone()).flatten()),
        }
    };
    let (fa, fb) = (given(&a.1.from)?, given(&b.1.from)?);
    a.1.from = a.1.from.take().or(fb);
    b.1.from = b.1.from.take().or(fa);
    let trig = clause(lw, st, a, ctx)?;
    let targ = clause(lw, st, b, ctx)?;
    Ok(Body::TrigTarg(trig, targ))
}

/// A trigger or target clause's signal (none for `at=`) and its `key=value` words.
fn head(words: &[String], ctx: Ctx) -> Result<(Option<Want>, Params), No> {
    let (first, rest) = words.split_first().ok_or(No::Invalid)?;
    if first.len() >= 2 && first[..2].eq_ignore_ascii_case("at") {
        return Ok((None, params(words, true)?));
    }
    Ok((Some(one(first, ctx)?), params(rest, true)?))
}

/// A trigger or target clause: `at=v`, or `sig [val=] [td=|from=] [to=] rise|fall|cross=n|last`.
fn clause(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    (signal, p): (Option<Want>, Params),
    ctx: Ctx,
) -> Result<Ev, No> {
    let Some(signal) = signal else {
        let extra = p.edge.is_some() || p.val.is_some() || p.td.is_some();
        if extra || p.from.is_some() || p.to.is_some() {
            return Err(No::Untyped);
        }
        return Ok(Ev::At(value(lw, st, &p.at.ok_or(No::Invalid)?)?));
    };
    if p.at.is_some() {
        return Err(No::Untyped); // a signal and at=: ngspice measures at
    }
    let value = match &p.val {
        Some(v) => value(lw, st, v)?,
        None => Val::Lit(ir::Literal::ZERO, number::Fit::Exact),
    };
    if p.edge.is_none() {
        return Err(No::Invalid); // "at, rise, fall or cross must be given"
    }
    crossing(lw, st, signal, value, p, ctx)
}

/// `sig=rhs [td=] [rise|fall|cross=n|last]`.
fn when(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String], ctx: Ctx) -> Result<Ev, No> {
    let (first, rest) = words.split_first().ok_or(No::Invalid)?;
    let (sig, rhs) = first.split_once('=').ok_or(No::Invalid)?;
    let signal = one(sig, ctx)?;
    let low = rhs.to_ascii_lowercase();
    let value = if low.starts_with("v(") || low.starts_with("i(") {
        if matches!(ctx, Ctx::Meas(AnalysisKind::Ac | AnalysisKind::Sp)) {
            return Err(No::Untyped); // compared by real part (get_value): not typed
        }
        // "Expressions like v(b)*2 are not supported"
        let w = one(rhs, Ctx::Meas(AnalysisKind::Tran)).map_err(|_| No::Invalid)?;
        if w.a.len() != 1 {
            return Err(No::Untyped);
        }
        Val::Probe(w)
    } else {
        value(lw, st, rhs)?
    };
    let p = params(rest, false)?;
    crossing(lw, st, signal, value, p, ctx)
}

/// `find sig at=v` or `find sig when …`.
fn find(lw: &mut Lower<'_, '_>, st: Stmt, words: &[String], ctx: Ctx) -> Result<Body, No> {
    let [sig, how, rest @ ..] = words else {
        return Err(No::Invalid);
    };
    let signal = one(sig, ctx)?;
    if how.eq_ignore_ascii_case("when") {
        return Ok(Body::Find(signal, when(lw, st, rest, ctx)?));
    }
    let p = params(std::slice::from_ref(how), true)?;
    match p.at {
        Some(at) if rest.is_empty() => Ok(Body::Find(signal, Ev::At(value(lw, st, &at)?))),
        Some(_) => Err(No::Untyped), // from=/to= after at=: ignored by measure_at, not typed
        None => Err(No::Invalid),    // "bad syntax of WHEN"
    }
}

/// `avg|min|max|pp|rms|integ sig [from=v] [to=v]`.
fn stat(lw: &mut Lower<'_, '_>, st: Stmt, f: &str, words: &[String], ctx: Ctx) -> Result<Body, No> {
    let (sig, rest) = words.split_first().ok_or(No::Invalid)?;
    let stat = match f {
        "avg" => Stat::Avg,
        "min" => Stat::Min,
        "max" => Stat::Max,
        "pp" => Stat::Pp,
        "rms" => Stat::Rms,
        "min_at" => Stat::MinAt,
        "max_at" => Stat::MaxAt,
        _ => Stat::Integ,
    };
    let signal = one(sig, ctx)?;
    let dc = ctx == Ctx::Meas(AnalysisKind::Dc);
    let (mut from, mut to) = (Ev::Bound, Ev::Bound);
    for w in rest {
        let (k, v) = w.split_once('=').ok_or(No::Invalid)?;
        match k.to_ascii_lowercase().as_str() {
            "from" => from = Ev::At(value(lw, st, v)?),
            // off dc `to=0` is "no limit" (`m_to != 0.0e0`): not the position 0
            "to" if !dc && number::card(v).is_some_and(|s| s.literal.mantissa == 0) => {
                return Err(No::Untyped);
            }
            "to" => to = Ev::At(value(lw, st, v)?),
            "td" | "val" | "rise" | "fall" | "cross" | "at" => return Err(No::Untyped),
            _ => return Err(No::Invalid), // "no such parameter"
        }
    }
    // dc: "make m_from always less than m_to" (measure_parse_stdParams), a swap of the two
    // values ngspice makes after substitution; the IR has no "either order" window
    if dc && let (Ev::At(a), Ev::At(b)) = (&from, &to) {
        match (a, b) {
            (Val::Lit(x, _), Val::Lit(y, _)) if y.value() < x.value() => {
                std::mem::swap(&mut from, &mut to);
            }
            (Val::Lit(..), Val::Lit(..)) => {}
            _ => return Err(No::DcWindow),
        }
    }
    Ok(Body::Stat(stat, signal, from, to))
}

/// Fails a `param=` measure that reads a `param=` measure of a later card: ngspice evaluates
/// them in card order (measure.c second pass), so the read is "Undefined parameter" (run: `p6`
/// failed); with a `.param` of that name it reads the parameter instead (run: `p4` = 7), which
/// this reader does not type, nor a deck whose includes may define one.
fn later_param(lw: &Lower<'_, '_>, st: Stmt, text: &str) -> Result<(), No> {
    let (mut later, mut params, mut includes) = (Vec::new(), Vec::new(), false);
    let mut after = false;
    for &n in lw.ast.fields(lw.ast.root()) {
        let n = Node(n);
        after |= n == st.node;
        match lw.ast.tag(n) {
            Tag::Measure if after && n != st.node => {
                let w = words(&lw.rest(n));
                let param = w.get(2).is_some_and(|f| {
                    let f = f.to_ascii_lowercase();
                    f.starts_with("param=") || f.starts_with("expr=")
                });
                if param {
                    later.push(name::key(&w[1]));
                }
            }
            Tag::Param => params.extend(
                words(&lw.rest(n))
                    .iter()
                    .filter_map(|w| w.split_once('='))
                    .map(|(k, _)| name::key(k)),
            ),
            Tag::Include | Tag::LibCall => includes = true,
            _ => {}
        }
    }
    for w in idents(text) {
        let k = name::key(w);
        if later.contains(&k) {
            return Err(if includes || params.contains(&k) {
                No::Untyped
            } else {
                No::Invalid
            });
        }
    }
    Ok(())
}

/// The names an expression's text reads.
fn idents(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.starts_with(|c: char| c.is_ascii_alphabetic()))
}

/// A crossing event. No edge given: ngspice's first crossing after the first interval
/// (`skip_initial`, R30; never `cross=1`). `td=` (tran only) or `from=` (tran, ac, sp: both skip
/// points below them in `com_measure_when`) is the delay, `to=` the end of the search; a dc
/// window (`from`/`to` swapped and defaulted to ±1e99) is not typed.
fn crossing(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    signal: Want,
    value: Val,
    p: Params,
    ctx: Ctx,
) -> Result<Ev, No> {
    let tran = ctx == Ctx::Meas(AnalysisKind::Tran);
    let dc = ctx == Ctx::Meas(AnalysisKind::Dc);
    if (p.td.is_some() && (!tran || p.from.is_some()))
        || (dc && (p.from.is_some() || p.to.is_some()))
    {
        return Err(No::Untyped); // TD off tran is ignored; TD with FROM is their maximum
    }
    if p.to
        .as_deref()
        .and_then(number::card)
        .is_some_and(|n| n.literal.mantissa == 0)
    {
        return Err(No::Untyped); // `to=0` is "no limit" (`m_to != 0.0e0`)
    }
    let skip_initial = p.edge.is_none();
    let (edge, count) = p.edge.unwrap_or((Edge::Cross, None));
    let count = count.map(|c| self::value(lw, st, &c)).transpose()?;
    let delay =
        p.td.or(p.from)
            .map(|t| self::value(lw, st, &t))
            .transpose()?;
    let until = p.to.map(|t| self::value(lw, st, &t)).transpose()?;
    Ok(Ev::Cross(Box::new(Crossing {
        signal,
        value,
        edge,
        last: count.is_none() && !skip_initial,
        count,
        skip_initial,
        delay,
        until,
    })))
}

/// A clause's `key=value` words (`measure_parse_stdParams`); `trig` allows `val=` and `at=`.
fn params(words: &[String], trig: bool) -> Result<Params, No> {
    let mut p = Params::default();
    for w in words {
        if w.eq_ignore_ascii_case("last") {
            p.edge = Some((Edge::Cross, None));
            continue;
        }
        let (k, v) = w.split_once('=').ok_or(No::Invalid)?; // "equal sign missing"
        let count = (!v.eq_ignore_ascii_case("last")).then(|| v.to_string());
        match k.to_ascii_lowercase().as_str() {
            "rise" => p.edge = Some((Edge::Rise, count)),
            "fall" => p.edge = Some((Edge::Fall, count)),
            "cross" => p.edge = Some((Edge::Cross, count)),
            "td" => p.td = Some(v.to_string()),
            "val" if trig => p.val = Some(v.to_string()),
            "at" if trig => p.at = Some(v.to_string()),
            "from" => p.from = Some(v.to_string()),
            "to" => p.to = Some(v.to_string()),
            "val" | "at" => return Err(No::Untyped),
            _ => return Err(No::Invalid), // "no such parameter"
        }
    }
    Ok(p)
}

/// The one probe a measure's signal word names.
fn one(word: &str, ctx: Ctx) -> Result<Want, No> {
    let mut wants = probe_word(word, ctx)?;
    if wants.len() != 1 || wants[0].kind == ProbeKind::Vdiff {
        return Err(No::Invalid);
    }
    Ok(wants.remove(0))
}

/// A value: a number as written, or a numparam expression (inpcom braces it).
fn value(lw: &mut Lower<'_, '_>, st: Stmt, text: &str) -> Result<Val, No> {
    let numeric = text
        .trim_start_matches(['-', '+'])
        .starts_with(|c: char| c.is_ascii_digit() || c == '.');
    if numeric {
        let s = number::card(text).ok_or(No::Invalid)?;
        return Ok(Val::Lit(s.literal, s.fit));
    }
    Ok(Val::Tree(numparam(lw, st, unwrap(text), true)?))
}

/// A numparam expression; `early` (a card value, substituted when the deck is read) may not
/// read a measure: ngspice stops on "Undefined parameter" (INV 14.08).
fn numparam(lw: &mut Lower<'_, '_>, st: Stmt, text: &str, early: bool) -> Result<Tree, No> {
    if early && reads_measure(lw, st, text) {
        return Err(No::Invalid);
    }
    if lw.class(st.container, text, &[]) >= Class::Temp {
        return Err(No::Invalid); // numparam has no v(), i(), time or temper
    }
    expr_read::parse_in(text, Eval::Numparam).map_err(|f| match f.class {
        ir::OpaqueClass::InvalidInSource => No::Invalid,
        _ => No::Untyped,
    })
}

/// Whether a name in `text` is a measure already defined in `st`'s scope.
fn reads_measure(lw: &mut Lower<'_, '_>, st: Stmt, text: &str) -> bool {
    for w in idents(text) {
        let id = lw.b.design().find_name(&lw.names.fold(w));
        if !id.is_none()
            && !lw
                .b
                .design()
                .lookup(st.scope, ir::DefKind::Measure, id)
                .is_none()
        {
            return true;
        }
    }
    false
}

/// An event appended.
fn event(lw: &mut Lower<'_, '_>, st: Stmt, e: Ev) -> Event {
    match e {
        Ev::Bound => Event::BOUND,
        Ev::At(v) => Event::at(emit(lw, st, v)),
        Ev::Cross(c) => {
            let Crossing {
                signal,
                value,
                edge,
                last,
                count,
                skip_initial,
                delay,
                until,
            } = *c;
            let signal = build(lw, &signal);
            let value = emit(lw, st, value);
            let count = count.map_or(ir::ExprId::NONE, |c| emit(lw, st, c));
            let delay = delay.map_or(ir::ExprId::NONE, |d| emit(lw, st, d));
            let until = until.map_or(ir::ExprId::NONE, |d| emit(lw, st, d));
            Event {
                last,
                skip_initial,
                delay,
                until,
                ..Event::cross(signal, edge, value, count)
            }
        }
    }
}

/// A value appended.
fn emit(lw: &mut Lower<'_, '_>, st: Stmt, v: Val) -> ir::ExprId {
    match v {
        Val::Lit(lit, fit) => lw.number(lit, &fit),
        Val::Tree(t) => lw.expr(st, &t, st.scope),
        Val::Probe(w) if w.kind == ProbeKind::I => {
            let e = lw.names.fold(&w.a[0]);
            lw.b.i(&e)
        }
        Val::Probe(w) => {
            let n = lw.names.node(&w.a[0]);
            lw.b.v(&n, None)
        }
    }
}
