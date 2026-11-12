//! The run rows' shared pieces: their IR-text words (an enum member ↔ its snake-case word, a
//! `key=value` field, a flag), their pools (append a list, read a list back range-checked), the
//! `FieldDomain` row, and the design scans the run rows' facts need (does an expression read the
//! circuit, where does an item sit). Row values in, text, lists, diagnostics and answers out.
//!
//! IR-text conventions every run row follows (`text.rs` grammar): fields in declaration order,
//! separated by one space; a `NONE` value, an empty list and an enum at its first (default)
//! member when the row says so are left out; an enum member is the snake case of its name
//! (`SpanOver50` → `span_over50`); a `bool` is a bare word, present when true; a compound value
//! (a probe, a target, a sweep, an event, a variation) is a parenthesized group of its own
//! fields. Expressions, names and references are written by the kernel's text writers.
//!
//! Cites: `crates/ir/src/text.rs` (grammar, `Words`); `rewrite/IR-IMPL.md` §1; IR-KERNEL C5, C11.
//!
//! Contents:
//! - words: `pub(crate) fn word`, `pub(crate) fn member`, `pub(crate) fn put_word`,
//!   `pub(crate) fn put_flag`, `pub(crate) fn trim_lead`, `pub(crate) fn put_expr`, `pub(crate) fn put_exprs`,
//!   `pub(crate) fn put_name`, `pub(crate) fn put_names`, `pub(crate) fn put_ref`, `pub(crate) fn write_ref`,
//!   `pub(crate) fn put_value`, `pub(crate) fn take_word`, `pub(crate) fn take_enum`,
//!   `pub(crate) fn take_key`, `pub(crate) fn take_expr`, `pub(crate) fn take_exprs`,
//!   `pub(crate) fn take_name`, `pub(crate) fn take_names`, `pub(crate) fn open`,
//!   `pub(crate) fn close`
//! - pools: `pub(crate) fn pool`, `pub(crate) fn get`
//! - facts: `pub(crate) fn flag`, `pub(crate) fn has`, `pub(crate) fn domain_unless`,
//!   `pub(crate) fn reads_circuit`, `pub(crate) fn is_one`, `pub(crate) fn is_zero`,
//!   `pub(crate) fn loop_depth`, `pub(crate) fn gen_name` (gen)

use std::fmt::{Debug, Write as _};

use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::expr::{Expr, Literal, NumForm};
use crate::feature::{Feature, FeatureSet};
use crate::id::{ExprId, ExprList, List, NameId, NameList, POOL_MAX, Ref, ScopeId};
use crate::item::{DefKind, ScopeKind};
use crate::run_extra::RunExtra;
use crate::text::Words;
use crate::text_expr::{self, Ctx};

// ---- words ----

/// The snake case of a member's name: an `_` before each capital but the first, lower-cased
/// (`AllVTop` → `all_v_top`, `Itl1` → `itl1`).
pub(crate) fn word<E: Debug>(e: E) -> String {
    let name = format!("{e:?}");
    let mut out = String::with_capacity(name.len() + 4);
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() && i > 0 {
            out.push('_');
        }
        out.push(c.to_ascii_lowercase());
    }
    out
}

/// The member of `all` whose word is `w`.
pub(crate) fn member<E: Copy + Debug>(all: &[E], w: &str) -> Option<E> {
    // One buffer for every candidate's name, compared with `w` as it is snake-cased (parse
    // calls this per enum field: no allocation per candidate).
    let mut name = String::with_capacity(24);
    all.iter().copied().find(|&e| {
        name.clear();
        write!(name, "{e:?}").expect("writing to a String does not fail");
        snake_eq(&name, w)
    })
}

/// ` w`.
pub(crate) fn put_word(out: &mut String, w: &str) {
    out.push(' ');
    out.push_str(w);
}

/// Removes the space a field writer put at `at`: a row's first field follows the kernel's
/// `keyword ` directly.
pub(crate) fn trim_lead(out: &mut String, at: usize) {
    if out[at..].starts_with(' ') {
        out.remove(at);
    }
}

/// ` w` when `on`.
pub(crate) fn put_flag(out: &mut String, on: bool, w: &str) {
    if on {
        put_word(out, w);
    }
}

/// ` key=value`, unless `e` is `NONE`.
pub(crate) fn put_expr(out: &mut String, design: &Design, ctx: &Ctx, key: &str, e: ExprId) {
    if !e.is_none() {
        put_word(out, key);
        out.push('=');
        put_value(out, design, ctx, e);
    }
}

/// ` key=[e1, e2]`, unless the list is empty.
pub(crate) fn put_exprs(out: &mut String, design: &Design, ctx: &Ctx, key: &str, list: ExprList) {
    if list.len == 0 {
        return;
    }
    put_word(out, key);
    out.push('=');
    text_expr::write_exprs(design, list, ctx, out);
}

/// ` key=name` (` name` when `key` is empty), unless `n` is `NONE`.
pub(crate) fn put_name(out: &mut String, design: &Design, key: &str, n: NameId) {
    if n.is_none() {
        return;
    }
    out.push(' ');
    if !key.is_empty() {
        out.push_str(key);
        out.push('=');
    }
    text_expr::write_name(design.str(n), out);
}

/// ` key=(a b)` (` (a b)` when `key` is empty), unless the list is empty.
pub(crate) fn put_names(out: &mut String, design: &Design, key: &str, list: NameList) {
    if list.len == 0 {
        return;
    }
    out.push(' ');
    if !key.is_empty() {
        out.push_str(key);
        out.push('=');
    }
    out.push('(');
    for (i, &n) in design.names(list).iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        text_expr::write_name(design.str(n), out);
    }
    out.push(')');
}

/// ` name`, ` name?` or ` name#k` for a reference (the kernel's spelling; a resolved reference
/// is bare unless lookup from `ctx.scope` binds another definition: `name#k`, A9).
pub(crate) fn put_ref(out: &mut String, design: &Design, ctx: &Ctx, r: Ref, kind: DefKind) {
    out.push(' ');
    write_ref(out, design, ctx, r, kind);
}

/// A reference, as `put_ref` writes it, without the leading space.
pub(crate) fn write_ref(out: &mut String, design: &Design, ctx: &Ctx, r: Ref, kind: DefKind) {
    text_expr::write_ref(design, r, kind, ctx, out);
}

/// A field value: an atom as is, any other expression in parentheses.
pub(crate) fn put_value(out: &mut String, design: &Design, ctx: &Ctx, e: ExprId) {
    text_expr::write_value(design, e, ctx, out);
}

/// Consumes `w` when it is the next word.
pub(crate) fn take_word(words: &mut Words<'_>, w: &str) -> bool {
    if words.peek() == Some(w) {
        words.next();
        return true;
    }
    false
}

/// The next word as a member of `all`; reports and returns `None` when it is not one.
pub(crate) fn take_enum<E: Copy + Debug>(
    words: &mut Words<'_>,
    all: &[E],
    diags: &mut Diags,
) -> Option<E> {
    let found = words.peek().and_then(|w| member(all, w));
    match found {
        Some(e) => {
            words.next();
            Some(e)
        }
        None => {
            words.error(diags);
            None
        }
    }
}

/// Consumes `key =` when `key` is the next field's key; consumes nothing otherwise.
pub(crate) fn take_key(words: &mut Words<'_>, key: &str) -> bool {
    let at = words.pos;
    if words.key() == Some(key) {
        return true;
    }
    words.pos = at;
    false
}

/// The value of an optional `key=value` field, or `NONE` when the field is absent.
pub(crate) fn take_expr(
    words: &mut Words<'_>,
    key: &str,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ExprId {
    if take_key(words, key) {
        return words.value(scope, b, diags);
    }
    ExprId::NONE
}

/// The list of an optional `key=[…]` field, or empty when the field is absent.
pub(crate) fn take_exprs(
    words: &mut Words<'_>,
    key: &str,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ExprList {
    if take_key(words, key) {
        return words.exprs(scope, b, diags);
    }
    List::EMPTY
}

/// The name of an optional `key=name` field, or `NONE` when absent.
pub(crate) fn take_name(
    words: &mut Words<'_>,
    key: &str,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> NameId {
    if take_key(words, key) {
        return words.name(b, diags);
    }
    NameId::NONE
}

/// The names of an optional `key=(…)` field (`key` empty: a positional `(…)`), or empty when
/// absent.
pub(crate) fn take_names(
    words: &mut Words<'_>,
    key: &str,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> NameList {
    let present = if key.is_empty() {
        words.peek() == Some("(")
    } else {
        take_key(words, key)
    };
    if !present {
        return List::EMPTY;
    }
    let list = words.names(b, diags);
    if list.len == 0 { List::EMPTY } else { list }
}

/// Consumes `(`; reports when it is not next.
pub(crate) fn open(words: &mut Words<'_>, diags: &mut Diags) -> bool {
    words.expect("(", diags)
}

/// Consumes `)`; reports when it is not next.
pub(crate) fn close(words: &mut Words<'_>, diags: &mut Diags) -> bool {
    words.expect(")", diags)
}

// ---- pools ----

/// Appends `items` to a run pool; the empty list is `List::EMPTY` (so a built list and a
/// parsed one compare equal). Asserts `POOL_MAX`.
pub(crate) fn pool<T: Copy>(pool: &mut Vec<T>, items: &[T]) -> List<T> {
    if items.is_empty() {
        return List::EMPTY;
    }
    let start = pool.len();
    assert!(
        start + items.len() <= POOL_MAX,
        "a run pool holds at most POOL_MAX elements"
    );
    pool.extend_from_slice(items);
    let list = List {
        start: start as u32,
        len: items.len() as u32,
        pool: Default::default(),
    };
    assert!(list.start as usize + list.len as usize == pool.len());
    list
}

/// The elements of `list`, or `None` when its range overruns the pool (a `HandleRange`).
pub(crate) fn get<T>(pool: &[T], list: List<T>) -> Option<&[T]> {
    let start = list.start as usize;
    pool.get(start..start.checked_add(list.len as usize)?)
}

// ---- facts ----

/// Adds `Feature::Run(x)` when `on`.
pub(crate) fn flag(set: &mut FeatureSet, on: bool, x: RunExtra) {
    if on {
        *set = set.with(Feature::Run(x));
    }
}

/// Whether `features` holds `Feature::Run(x)`.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn has(features: &FeatureSet, x: RunExtra) -> bool {
    features.contains(Feature::Run(x))
}

/// `FieldDomain` unless `ok`.
pub(crate) fn domain_unless(ok: bool) -> Option<Violation> {
    (!ok).then_some(Violation::FieldDomain)
}

/// Whether the tree at `e` holds a `V`, `I`, `Time` or `Freq` leaf or a `Ddt`/`Idt` call.
/// Explicit stack (C11). `NONE` reads nothing.
pub(crate) fn reads_circuit(design: &Design, e: ExprId) -> bool {
    if e.is_none() {
        return false;
    }
    let mut stack = vec![e];
    let mut seen = 0usize;
    while let Some(x) = stack.pop() {
        seen += 1;
        assert!(seen <= design.expr_count(), "an expression tree is acyclic");
        match design.expr(x) {
            Expr::V(..) | Expr::I(_) | Expr::Time | Expr::Freq => return true,
            Expr::Call(f, list) => {
                if f.reads_circuit() {
                    return true;
                }
                stack.extend_from_slice(design.exprs(list));
            }
            Expr::UserCall(_, list) | Expr::Draw(_, _, _, list) | Expr::List(list) => {
                stack.extend_from_slice(design.exprs(list));
            }
            Expr::Unary(_, a) => stack.push(a),
            Expr::Binary(_, a, c) => stack.extend([a, c]),
            Expr::Ternary(c, t, o) => stack.extend([c, t, o]),
            Expr::Num(_)
            | Expr::Str(_)
            | Expr::Param(_)
            | Expr::Temp
            | Expr::TStep
            | Expr::TStop
            | Expr::Tnom => {}
        }
    }
    false
}

/// Whether `e` is a literal equal to 1.
pub(crate) fn is_one(design: &Design, e: ExprId) -> bool {
    if e.is_none() {
        return false;
    }
    let one = Literal {
        mantissa: 1,
        frac: 0,
        exp10: 0,
        form: NumForm::Plain,
    };
    matches!(design.expr(e), Expr::Num(k) if design.literal_eq(design.num(k), one))
}

/// Whether `e` is the literal 0, of any form (`0`, `0.0`, `0e3`, the integer 0, a `Long` of
/// zero digits); `NONE` is not.
pub(crate) fn is_zero(design: &Design, e: ExprId) -> bool {
    if e.is_none() {
        return false;
    }
    let Expr::Num(k) = design.expr(e) else {
        return false;
    };
    let l = design.num(k);
    match design.long_digits(l) {
        // A18: a `Long` literal's digits are the design's
        Some((digits, _)) => digits.bytes().all(|c| !c.is_ascii_digit() || c == b'0'),
        None => l.mantissa == 0,
    }
}

/// How many `Loop` scopes enclose `scope`, itself included. Bounded by the scope count.
pub(crate) fn loop_depth(design: &Design, scope: ScopeId) -> usize {
    let mut n = 0;
    let mut s = scope;
    for _ in 0..=design.scopes.len() {
        if s.is_none() {
            return n;
        }
        let row = design.scope(s);
        if row.kind == ScopeKind::Loop {
            n += 1;
        }
        s = row.parent;
    }
    unreachable!("the scope tree has a cycle")
}

/// A generated statement name: a fresh one or `NONE`, as the set's `named` and `unnamed`
/// features allow (named one time in `n` when both do); `None` when neither does.
#[cfg(any(test, feature = "gen"))]
pub(crate) fn gen_name(
    g: &mut crate::r#gen::Gen,
    b: &mut Builder<'_>,
    features: &FeatureSet,
    named: RunExtra,
    unnamed: RunExtra,
    n: u32,
) -> Option<NameId> {
    let name = match (has(features, named), has(features, unnamed)) {
        (false, false) => return None,
        (true, false) => true,
        (false, true) => false,
        (true, true) => g.chance(n),
    };
    Some(if name { g.fresh_name(b) } else { NameId::NONE })
}

// ---- private ----

/// Whether `w` is `name`'s snake case (`word`'s rule), without building it.
fn snake_eq(name: &str, w: &str) -> bool {
    let mut want = w.bytes();
    for (i, c) in name.bytes().enumerate() {
        if c.is_ascii_uppercase() && i > 0 && want.next() != Some(b'_') {
            return false;
        }
        if want.next() != Some(c.to_ascii_lowercase()) {
            return false;
        }
    }
    want.next().is_none()
}
