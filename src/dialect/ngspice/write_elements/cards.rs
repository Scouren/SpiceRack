//! One element → its ngspice card text (`card`; whether it has one is `admit.rs`). Each kind's
//! spelling is the form its reader in
//! `read_elements/` reads back to the same rows: value right after the nodes, then the master,
//! then the typed parameters in `Passive`/`Semi`/`Line` field order, then the instance
//! parameters, then `m`.
//!
//! Contents: `pub(super) fn card(&ir::Design, &ir::Element) -> String`,
//! `pub(super) fn names_run(&ir::Design, ir::ExprId) -> bool`,
//! `pub(super) fn passive_fields(&Passive) -> [ir::ExprId; 8]`

use super::super::expr_write;
use super::super::number::{self, Grammar};
use super::super::tables::Eval;
use crate::ir::{self, ElemKind as K, Expr, ExprId};

/// The card of `e` (a design `spellable` accepted).
pub(super) fn card(d: &ir::Design, e: &ir::Element) -> String {
    let mut s = String::from(d.str(e.name));
    for &t in d.names(e.terminals) {
        s.push(' ');
        s.push_str(d.str(t));
    }
    match e.kind {
        K::Resistor | K::Capacitor | K::Inductor => passive(d, e, &mut s),
        K::ChargeCapacitor => {
            let b = d.behav(e);
            s.push_str(" q=");
            bv(d, b.expr, &mut s);
            kv(d, "tc1", b.tc1, &mut s);
            kv(d, "tc2", b.tc2, &mut s);
            mult(d, e, &mut s);
        }
        K::Coupling => {
            let c = d.coupling(e);
            for &l in d.names(c.inductors) {
                s.push(' ');
                s.push_str(d.str(l));
            }
            s.push(' ');
            cv(d, c.k, &mut s);
        }
        K::Vcvs | K::Vccs | K::Cccs | K::Ccvs => {
            let c = d.controlled(e);
            if !c.sense.is_none() {
                s.push(' ');
                s.push_str(d.str(c.sense));
            }
            s.push(' ');
            cv(d, c.gain, &mut s);
            mult(d, e, &mut s);
        }
        K::Poly => {
            let p = d.poly(e);
            s.push_str(&format!(" poly({})", p.dimension));
            for &c in d.names(p.controls) {
                s.push(' ');
                s.push_str(d.str(c));
            }
            for &c in d.exprs(p.coeffs) {
                s.push(' ');
                cv(d, c, &mut s);
            }
        }
        K::Table => {
            let t = d.table_source(e);
            s.push_str(" table ");
            bv(d, t.input, &mut s);
            s.push_str(" =");
            for xy in d.exprs(t.points).chunks(2) {
                s.push_str(" (");
                cv(d, xy[0], &mut s);
                s.push_str(", ");
                cv(d, xy[1], &mut s);
                s.push(')');
            }
            mult(d, e, &mut s);
        }
        K::Behav => behav(d, e, &mut s),
        K::Diode | K::Bjt | K::Mosfet | K::Jfet | K::Mesfet => semi(d, e, &mut s),
        K::VSwitch | K::ISwitch => {
            let w = d.switch(e);
            if !w.sense.is_none() {
                s.push(' ');
                s.push_str(d.str(w.sense));
            }
            master(d, e, &mut s);
            match w.initial {
                ir::semi::InitialState::Unset => {}
                ir::semi::InitialState::Off => s.push_str(" off"),
                ir::semi::InitialState::On => s.push_str(" on"),
            }
        }
        K::LosslessLine | K::LossyLine | K::RcLine | K::SingleLossyLine | K::CoupledLines => {
            line(d, e, &mut s);
        }
        K::CodeModel | K::ModuleModel | K::Instance => {
            master(d, e, &mut s);
            params(d, e, &mut s);
            mult(d, e, &mut s);
        }
        K::FluxInductor
        | K::VSource
        | K::ISource
        | K::Port
        | K::Laplace
        | K::FreqTable
        | K::Delay
        | K::Igbt
        | K::Module
        | K::Unresolved => unreachable!("ngspice write: {:?} is not in FEATURES", e.kind),
    }
    s
}

/// Whether the expression holds `v(`, `i(`, `time`, `hertz` or `temper` itself (what makes
/// ngspice run an R/C/L as a B source, `inpcom.c` `b_transformation_wanted`).
pub(super) fn names_run(d: &ir::Design, root: ExprId) -> bool {
    if root.is_none() {
        return false;
    }
    let mut stack = vec![root];
    // Bounded: children have lower ids than their parent; each edge is pushed once.
    while let Some(x) = stack.pop() {
        match d.expr(x) {
            Expr::V(..) | Expr::I(_) | Expr::Time | Expr::Freq | Expr::Temp => return true,
            Expr::Call(_, a) | Expr::UserCall(_, a) | Expr::Draw(_, _, _, a) | Expr::List(a) => {
                stack.extend_from_slice(d.exprs(a));
            }
            Expr::Unary(_, a) => stack.push(a),
            Expr::Binary(_, a, b) => stack.extend([a, b]),
            Expr::Ternary(c, t, o) => stack.extend([c, t, o]),
            Expr::Num(_)
            | Expr::Str(_)
            | Expr::Param(_)
            | Expr::TStep
            | Expr::TStop
            | Expr::Tnom => {}
        }
    }
    false
}

/// The fields of a passive row that ngspice's B-source rewrite keeps or drops, in field order.
pub(super) fn passive_fields(p: &ir::passive::Passive) -> [ExprId; 8] {
    [
        p.ic, p.tc1, p.tc2, p.tce, p.temp_abs, p.dtemp, p.ac, p.scale,
    ]
}

// ---- private ----

/// Appends a card value: a number in `INPevaluate`'s spelling, anything else in `{…}`.
fn cv(d: &ir::Design, e: ExprId, out: &mut String) {
    if let Expr::Num(n) = d.expr(e) {
        return number::spell(d, d.num(n), Grammar::Inp, out);
    }
    out.push('{');
    expr_write::expr(d, e, out);
    out.push('}');
}

/// Appends a B-source field in `{…}`, in the B-source grammar.
fn bv(d: &ir::Design, e: ExprId, out: &mut String) {
    out.push('{');
    expr_write::expr_in(d, e, Eval::Behavioural, out);
    out.push('}');
}

/// ` key=value` when `e` is set.
fn kv(d: &ir::Design, key: &str, e: ExprId, out: &mut String) {
    if !e.is_none() {
        out.push(' ');
        out.push_str(key);
        out.push('=');
        cv(d, e, out);
    }
}

/// ` model` when the element names one.
fn master(d: &ir::Design, e: &ir::Element, out: &mut String) {
    if !e.model.name.is_none() {
        out.push(' ');
        out.push_str(d.str(e.model.name));
    }
}

/// The instance parameters, ` k=v` each.
fn params(d: &ir::Design, e: &ir::Element, out: &mut String) {
    for &(k, v) in d.params(e.params) {
        kv(d, d.str(k), v, out);
    }
}

/// ` m=…` when the element has a multiplicity.
fn mult(d: &ir::Design, e: &ir::Element, out: &mut String) {
    kv(d, "m", e.mult, out);
}

/// An R, C or L: value, model, typed fields, parameters, `m`.
fn passive(d: &ir::Design, e: &ir::Element, s: &mut String) {
    let p = d.passive(e);
    let rewritten = names_run(d, p.value);
    if !p.value.is_none() {
        s.push(' ');
        if rewritten {
            bv(d, p.value, s);
        } else {
            cv(d, p.value, s);
        }
    }
    master(d, e, s);
    let keys = ["ic", "tc1", "tc2", "tce", "temp", "dtemp", "ac", "scale"];
    for (k, v) in keys.iter().zip(passive_fields(&p)) {
        kv(d, k, v, s);
    }
    match (rewritten, p.noisy) {
        (true, true) => s.push_str(" noisy=1"),
        (false, false) => s.push_str(" noisy=0"),
        _ => {}
    }
    params(d, e, s);
    mult(d, e, s);
}

/// A behavioural source by its letter: `B … v=|i=`, `E … vol=`, `G … cur=`.
fn behav(d: &ir::Design, e: &ir::Element, s: &mut String) {
    let b = d.behav(e);
    let voltage = b.output == ir::controlled::OutKind::Voltage;
    let key = match d.str(e.name).as_bytes()[0].to_ascii_lowercase() {
        b'e' => " vol=",
        b'g' => " cur=",
        _ if voltage => " v=",
        _ => " i=",
    };
    s.push_str(key);
    bv(d, b.expr, s);
    kv(d, "tc1", b.tc1, s);
    kv(d, "tc2", b.tc2, s);
    kv(d, "temp", b.temp_abs, s);
    kv(d, "dtemp", b.dtemp, s);
    mult(d, e, s);
}

/// A D, Q, M, J or Z: model, `area=`, `off`, `ic=a,b`, `temp=`, `dtemp=`, parameters, `m`.
fn semi(d: &ir::Design, e: &ir::Element, s: &mut String) {
    let x = d.semi(e);
    master(d, e, s);
    kv(d, "area", x.area, s);
    if x.initial == ir::semi::InitialState::Off {
        s.push_str(" off");
    }
    for (k, &v) in d.exprs(x.ic).iter().enumerate() {
        s.push_str(if k == 0 { " ic=" } else { "," });
        cv(d, v, s);
    }
    kv(d, "temp", x.temp_abs, s);
    kv(d, "dtemp", x.dtemp, s);
    params(d, e, s);
    mult(d, e, s);
}

/// A line: model, then its fields by ngspice's keys.
fn line(d: &ir::Design, e: &ir::Element, s: &mut String) {
    let l = d.line(e);
    master(d, e, s);
    kv(d, "z0", l.z0, s);
    kv(d, "td", l.td, s);
    kv(d, "f", l.freq, s);
    kv(d, "nl", l.nl, s);
    let len = if e.kind == K::RcLine { "l" } else { "len" };
    kv(d, len, l.length, s);
    kv(d, "n", l.lumps, s);
}
