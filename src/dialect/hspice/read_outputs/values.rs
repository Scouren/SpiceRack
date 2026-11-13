//! A measure's values, read but not yet appended, and the checks ngspice makes on them: a
//! number as written (its tail gobbled, `ft_numparse`), a numparam expression (`inpcom.c`
//! braces any other right-hand side), a `when` threshold's own rules, and the names a
//! `param=` measure may read (`measure.c` `do_measure`'s second pass). In: words; out: `Val`
//! or the `No` the card becomes; `emit` appends a `Val`.
//!
//! Contents: `pub(super) enum Val`, `pub(super) fn value`, `numparam`, `threshold`,
//! `param_reads`, `emit`

use super::No;
use super::probe::Want;
use super::words;
use crate::dialect::hspice::declare::Class;
use crate::dialect::hspice::expr_read::{self, Tree};
use crate::dialect::hspice::name;
use crate::dialect::hspice::number;
use crate::dialect::hspice::read::{Lower, Stmt};
use crate::dialect::hspice::value::{self, unwrap};
use crate::ir::{self, ProbeKind};
use crate::syntax::ast::{Node, Tag};

/// A measure value read but not yet appended.
pub(super) enum Val {
    /// A card value (the reader's one [`value::Val`]).
    Plain(value::Val),
    /// A threshold that reads the circuit: `v(x)` or `i(x)`.
    Probe(Want),
}

/// A value: a number as written, or a numparam expression (inpcom braces it).
pub(super) fn value(lw: &mut Lower<'_, '_>, st: Stmt, text: &str) -> Result<Val, No> {
    let numeric = text
        .trim_start_matches(['-', '+'])
        .starts_with(|c: char| c.is_ascii_digit() || c == '.');
    if numeric {
        let s = number::card(text).ok_or(No::Invalid)?;
        return Ok(Val::Plain(value::Val::of(s)));
    }
    let inner = unwrap(text).plain().unwrap_or(text.trim());
    Ok(Val::Plain(value::Val::Tree(numparam(lw, st, inner, true)?)))
}

/// A numparam expression; `early` (a card value, substituted when the deck is read) may not
/// read a measure: ngspice stops on "Undefined parameter" (INV 14.08).
pub(super) fn numparam(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    text: &str,
    early: bool,
) -> Result<Tree, No> {
    if early && reads_measure(lw, st, text) {
        return Err(No::Invalid);
    }
    if lw.class(st.container, text, &[]) >= Class::Temp {
        return Err(No::Invalid); // numparam has no v(), i(), time or temper
    }
    expr_read::parse(text).map_err(|f| match f.class {
        ir::OpaqueClass::InvalidInSource => No::Invalid,
        _ => No::Untyped,
    })
}

/// A `when` right-hand side that is not a probe: ngspice refuses an operator in it ("Expressions
/// like 0.2*2 are not supported", `str_has_arith_char2`; run) and, after a number and its scale,
/// anything but one unit letter of `avfc` (`measure_parse_when`'s `INPevaluate2` check). A
/// number's own tail is otherwise gobbled, as every other measure value is (run: `val=0.2*2`
/// reads 0.2).
pub(super) fn threshold(rhs: &str) -> Result<(), No> {
    if rhs.to_ascii_lowercase().contains("par(") {
        return Err(No::Untyped); // inpcom.c makes par() a B source and v(pa_xx) before this
    }
    let body = rhs.strip_prefix(['+', '-']).unwrap_or(rhs);
    if body.contains(|c: char| "*/<>?:|&^!%\\".contains(c)) {
        return Err(No::Invalid);
    }
    if !body.starts_with(|c: char| c.is_ascii_digit() || c == '.') {
        return Ok(()); // a numparam expression (inpcom braces it); its own rules apply
    }
    let low = body.to_ascii_lowercase();
    let tail = low.trim_start_matches(|c: char| !c.is_ascii_alphabetic() || c == 'e');
    let digits = low.len() - tail.len();
    let exp_ok = low[..digits]
        .chars()
        .all(|c| c.is_ascii_digit() || ".e+-".contains(c));
    let unit = [
        "meg", "mil", "t", "g", "k", "m", "u", "n", "p", "f", "a", "",
    ]
    .iter()
    .find_map(|sc| {
        tail.strip_prefix(sc)
            .filter(|r| r.is_empty() || (r.len() == 1 && "avfc".contains(*r)))
    });
    if !exp_ok || unit.is_none() {
        return Err(No::Invalid); // "Cannot evaluate"
    }
    Ok(())
}

/// Fails a `param=` measure reading a name ngspice has no value for when it evaluates it:
/// numparam runs the `param=` measures in card order after every other measure (measure.c
/// second pass), so a later `param=` measure is "Undefined parameter" (run: `p6` failed), and
/// so is a name no measure and no `.param` defines (runs: `fval`, and `pi`, `e`: numparam has
/// no constants). A `.param` of a later param measure's name is read instead (run: `p4` = 7),
/// which this reader does not type, nor a deck whose includes may define names.
pub(super) fn param_reads(lw: &Lower<'_, '_>, st: Stmt, text: &str) -> Result<(), No> {
    let (mut later, mut measures, mut params, mut includes) =
        (Vec::new(), Vec::new(), Vec::new(), false);
    let mut after = false;
    for &n in lw.ast.fields(lw.ast.root()) {
        let n = Node(n);
        let here = n == st.node;
        after |= here;
        match lw.ast.tag(n) {
            Tag::Measure if !here => {
                let w = words(&lw.rest(n));
                let param = w.get(2).is_some_and(|f| {
                    let f = f.to_ascii_lowercase();
                    f.starts_with("param=") || f.starts_with("expr=")
                });
                if let Some(name) = w.get(1) {
                    let k = name::key(name);
                    if param && after {
                        later.push(k);
                    } else {
                        measures.push(k);
                    }
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
        let defined = params.contains(&k);
        if later.contains(&k) && !measures.contains(&k) {
            return Err(if includes || defined {
                No::Untyped
            } else {
                No::Invalid
            });
        }
        if !includes && !defined && !measures.contains(&k) {
            return Err(No::Invalid);
        }
    }
    Ok(())
}

/// A value appended.
pub(super) fn emit(lw: &mut Lower<'_, '_>, st: Stmt, v: Val) -> ir::ExprId {
    match v {
        Val::Plain(v) => value::emit(lw, st, &v),
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

// ---- private ----

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

/// The names an expression's text reads (a name followed by `(` is a function, not read).
fn idents(text: &str) -> Vec<&str> {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut out = Vec::new();
    let mut at = 0;
    // Bounded: each step moves past one run of characters.
    while let Some(start) = text[at..].find(ident).map(|k| at + k) {
        let end = text[start..]
            .find(|c: char| !ident(c))
            .map_or(text.len(), |k| start + k);
        let call = text[end..].trim_start().starts_with('(');
        if text[start..].starts_with(|c: char| c.is_ascii_alphabetic()) && !call {
            out.push(&text[start..end]);
        }
        at = end;
    }
    out
}
