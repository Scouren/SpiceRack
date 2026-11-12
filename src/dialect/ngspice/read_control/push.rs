//! A statement's operations → IR items: [`ready`] reads every word into a value (an analysis
//! through the run area's `read_analysis`, a number through `number::scan`, an `alterparam`
//! value through numparam) with nothing appended, an operation that fails becoming its own
//! opaque; [`push`] appends them in order. Validate first, then push: a refusal leaves no row.
//!
//! Cites: `rewrite/S3-CONTROL.md` S3, S4, V1, L1; `rewrite/RULINGS.md` R1 (the literal as
//! written), R3 (the reader binds).
//!
//! Contents: `pub(super) enum Ready`, `pub(super) enum Value`, `pub(super) fn ready(&mut Lower, Stmt, &[Op]) ->
//! Vec<Ready>`, `pub(super) fn push(&mut Lower, Stmt, ir::ScopeId, &[Ready])`

use super::plan::{Op, Target};
use super::{FORM, OWN};
use crate::dialect::ngspice::expr_read::{self, Tree};
use crate::dialect::ngspice::number::{self, Grammar};
use crate::dialect::ngspice::read::{Lower, Stmt};
use crate::dialect::ngspice::read_outputs;
use crate::dialect::ngspice::read_run;
use crate::dialect::ngspice::tables::Eval;
use crate::ir;

/// An operation read, nothing appended yet.
pub(super) enum Ready {
    /// An analysis: its command word and words, read once by `read_analysis` already.
    // ponytail: read again at the push (the run area's `Card` is not exported yet)
    Analysis(String, Vec<String>),
    /// An alter: its target and value.
    Alter(Target, Value),
    /// A typed option, from here.
    Option(ir::OptionKey, Option<Value>),
    /// The temperature and the nominal one, from here (either may be absent).
    Env(Option<Value>, Option<Value>),
    /// A load.
    Load(String),
    /// A `save` or `meas` command the outputs area reads: which, its words, its opaque text.
    Output(bool, Vec<String>, String),
    /// An opaque: class, code, text.
    Opaque(ir::OpaqueClass, ir::Code, String),
    /// A loop: its variable, target, values and body.
    Loop(String, Target, Vec<ir::Literal>, Vec<Ready>),
}

/// A value read but not appended.
pub(super) enum Value {
    /// A number as written.
    Lit(ir::Literal),
    /// A numparam expression (an `alterparam` value).
    Expr(Tree),
}

/// Every operation of `ops` read, each failure its own opaque at its place.
pub(super) fn ready(lw: &mut Lower<'_, '_>, st: Stmt, ops: &[Op]) -> Vec<Ready> {
    let mut out = Vec::with_capacity(ops.len());
    for op in ops {
        let cmd = match op {
            Op::Analysis { cmd, .. }
            | Op::Alter { cmd, .. }
            | Op::State { cmd, .. }
            | Op::Load { cmd, .. }
            | Op::Save { cmd, .. }
            | Op::Meas { cmd, .. }
            | Op::Opaque { cmd, .. }
            | Op::Pending { cmd, .. }
            | Op::Loop { cmd, .. } => cmd,
        };
        match one(lw, st, op) {
            Ok(r) => out.extend(r),
            Err((class, code)) => out.push(Ready::Opaque(class, code, cmd.opaque_text())),
        }
    }
    out
}

/// Appends `items` to `scope` in order.
pub(super) fn push(lw: &mut Lower<'_, '_>, st: Stmt, scope: ir::ScopeId, items: &[Ready]) {
    let span = lw.span(st.node);
    for r in items {
        match r {
            Ready::Analysis(name, words) => {
                let inner = Stmt { scope, ..st };
                let card = read_run::read_analysis(lw, inner, name, words)
                    .unwrap_or_else(|_| unreachable!("ready read the analysis"));
                read_run::push_analysis(lw, inner, &card);
            }
            Ready::Alter(target, value) => {
                let t = target_of(lw, target);
                let v = emit(lw, st, scope, value);
                let item = lw.b.alter(scope, t, v, span);
                refer(lw, item, scope, target);
            }
            Ready::Option(key, value) => {
                let v = value
                    .as_ref()
                    .map_or(ir::ExprId::NONE, |v| emit(lw, st, scope, v));
                lw.b.option(scope, *key, v, span);
            }
            Ready::Env(temp, tnom) => {
                let t = temp
                    .as_ref()
                    .map_or(ir::ExprId::NONE, |v| emit(lw, st, scope, v));
                let n = tnom
                    .as_ref()
                    .map_or(ir::ExprId::NONE, |v| emit(lw, st, scope, v));
                lw.b.env(scope, t, n, span);
            }
            Ready::Load(path) => {
                lw.b.load(scope, ir::LoadKind::Osdi, path, span);
            }
            Ready::Output(save, words, text) => {
                // the outputs area validates before it pushes; a refusal is this command's opaque
                let inner = Stmt { scope, ..st };
                let done = if *save {
                    read_outputs::save_command(lw, inner, words)
                } else {
                    read_outputs::meas_command(lw, inner, words)
                };
                if let Err(no) = done {
                    let f = read_outputs::fail(no);
                    lw.b.opaque(scope, "ngspice", f.class, text, span);
                    lw.diags.push(f.code, span, 0);
                }
            }
            Ready::Opaque(class, code, text) => {
                lw.b.opaque(scope, "ngspice", *class, text, span);
                lw.diags.push(*code, span, 0);
            }
            Ready::Loop(var, target, values, body) => {
                lw.b.name(var); // interned before its target, as the IR text names them
                let t = target_of(lw, target);
                let vals: Vec<ir::ExprId> = values.iter().map(|&l| lw.b.literal(l)).collect();
                let list = lw.b.exprs(&vals);
                let row = ir::plan::SweepLoop {
                    target: t,
                    sweep: ir::SweepSpec::list(list),
                };
                let (item, inner) =
                    lw.b.loop_(scope, Some(var), row, ir::LoopEnd::KeepLast, span);
                refer(lw, item, scope, target);
                push(lw, st, inner, body);
            }
        }
    }
}

// ---- private ----

/// One operation read; `Err` is the class and code of its own opaque.
fn one(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    op: &Op,
) -> Result<Vec<Ready>, (ir::OpaqueClass, ir::Code)> {
    let fail = |f: expr_read::Fail| (f.class, f.code);
    Ok(vec![match op {
        Op::Analysis { name, rest, .. } => {
            let words = read_run::split_words(rest);
            read_run::read_analysis(lw, st, name, &words).map_err(fail)?;
            Ready::Analysis(name.clone(), words)
        }
        Op::Alter {
            target: Target::Param(p),
            value,
            ..
        } => {
            let text = value
                .strip_prefix('{')
                .and_then(|v| v.strip_suffix('}'))
                .unwrap_or(value);
            let tree = expr_read::parse_in(text, Eval::Numparam).map_err(fail)?;
            Ready::Alter(Target::Param(p.clone()), Value::Expr(tree))
        }
        Op::Alter { target, value, .. } => Ready::Alter(
            target.clone(),
            Value::Lit(literal(value).ok_or((ir::OpaqueClass::Unread, FORM))?),
        ),
        Op::State { keys, .. } => return state(keys),
        Op::Load { path, .. } => Ready::Load(path.clone()),
        Op::Save { rest, cmd } => Ready::Output(true, read_outputs::words(rest), cmd.opaque_text()),
        Op::Meas { rest, cmd } => {
            Ready::Output(false, read_outputs::words(rest), cmd.opaque_text())
        }
        Op::Opaque { class, code, cmd } => Ready::Opaque(*class, *code, cmd.opaque_text()),
        Op::Pending { .. } => unreachable!("plan settles every pending alterparam"),
        Op::Loop {
            var,
            target,
            values,
            body,
            ..
        } => {
            let vals = values
                .iter()
                .map(|w| literal(w))
                .collect::<Option<Vec<_>>>()
                .ok_or((ir::OpaqueClass::Unread, FORM))?;
            let inner = ready(lw, st, body);
            Ready::Loop(var.clone(), target.clone(), vals, inner)
        }
    }])
}

/// A script `option`/`set`: one `Option` per typed key, `temp`/`tnom` one `Env`.
fn state(keys: &[(String, Option<String>)]) -> Result<Vec<Ready>, (ir::OpaqueClass, ir::Code)> {
    let own = (ir::OpaqueClass::Private, OWN);
    let lit = |v: &Option<String>| v.as_deref().and_then(literal).map(Value::Lit).ok_or(own);
    let mut out = Vec::new();
    let (mut temp, mut tnom) = (None, None);
    for (k, v) in keys {
        match k.as_str() {
            "temp" => temp = Some(lit(v)?),
            "tnom" => tnom = Some(lit(v)?),
            "method" => {
                let key = match v.as_deref().map(str::to_ascii_lowercase).as_deref() {
                    Some("trap") | Some("trapezoidal") => ir::OptionKey::MethodTrap,
                    Some("gear") => ir::OptionKey::MethodGear,
                    _ => return Err(own),
                };
                out.push(Ready::Option(key, None));
            }
            _ => {
                let (_, key) = read_run::KEYS.iter().find(|(n, _)| n == k).ok_or(own)?;
                if key.is_flag() != v.is_none() {
                    return Err(own);
                }
                let value = if v.is_some() { Some(lit(v)?) } else { None };
                out.push(Ready::Option(*key, value));
            }
        }
    }
    if temp.is_some() || tnom.is_some() {
        out.push(Ready::Env(temp, tnom));
    }
    Ok(out)
}

/// A whole word as one card number (R1: its digits as written).
fn literal(word: &str) -> Option<ir::Literal> {
    let (negative, digits) = match word.as_bytes().first() {
        Some(b'-') => (true, &word[1..]),
        Some(b'+') => (false, &word[1..]),
        _ => (false, word),
    };
    let s = number::scan(digits, Grammar::Inp).filter(|s| s.len == digits.len())?;
    let mut lit = s.literal;
    if negative && lit.mantissa != 0 {
        lit.mantissa = -lit.mantissa;
    }
    Some(lit)
}

/// Appends a value.
fn emit(lw: &mut Lower<'_, '_>, st: Stmt, scope: ir::ScopeId, v: &Value) -> ir::ExprId {
    match v {
        Value::Lit(l) => lw.b.literal(*l),
        Value::Expr(tree) => lw.expr(st, tree, scope),
    }
}

/// The IR target of `t`, names folded (ngspice folds a control line).
fn target_of(lw: &mut Lower<'_, '_>, t: &Target) -> ir::Target {
    use ir::TargetKind as K;
    match t {
        Target::Element(e) => {
            let e = lw.names.fold(e);
            lw.b.target(K::ElementValue, &[], &e, None)
        }
        Target::Source(e) => {
            let e = lw.names.fold(e);
            lw.b.target(K::SourceDc, &[], &e, None)
        }
        Target::Device(e, p) => {
            let e = lw.names.fold(e);
            lw.b.target(K::DeviceParam, &[], &e, Some(p))
        }
        Target::Model(m, p) => {
            let m = lw.names.fold(m);
            lw.b.target(K::ModelParam, &[], &m, Some(p))
        }
        Target::Param(p) => {
            let p = lw.names.fold(p);
            lw.b.target(K::Param, &[], &p, None)
        }
    }
}

/// Queues the target's reference (slot 0 of an `Alter` and of a sweep `Loop`) for the bind
/// step: a parameter or a model, nearest `scope` outward (R3).
fn refer(lw: &mut Lower<'_, '_>, item: ir::ItemRef, scope: ir::ScopeId, t: &Target) {
    let (kind, name) = match t {
        Target::Param(p) => (ir::DefKind::Param, p),
        Target::Model(m, _) => (ir::DefKind::Model, m),
        Target::Element(_) | Target::Source(_) | Target::Device(..) => return,
    };
    let folded = lw.names.fold(name);
    let id = lw.b.name(&folded);
    lw.refer(item, 0, scope, kind, id);
}
