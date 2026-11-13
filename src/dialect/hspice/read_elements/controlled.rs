//! E, F, G and H cards → `Vcvs`, `Vccs`, `Cccs`, `Ccvs`, `Poly`, `Table` and `Behav` rows. HSPICE
//! has no B source (`read_elements.rs` refuses a B card): a behavioural source is `E … vol=`,
//! `E … value=`, `G … cur=`, `G … value=` (settle `e_forms.sp`). The words `vcvs`, `vccs`,
//! `cccs`, `ccvs` after the nodes are the linear forms (`e_forms.sp`), `poly(1)` the polynomial
//! (`e_poly.sp`). The rest of the rules are the ngspice dialect's, leads not yet settled on
//! ESPice: `m`, `table`, `laplace`.
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §4 (branch `differences`); settle decks above (ESPice 0.3.0
//! in HSPICE mode: ESPice is not HSPICE).
//!
//! Contents: `pub(super) fn efgh(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item], text: &str)`

use super::super::read::{Lower, Stmt};
use super::super::value::{Wrap, unwrap};
use super::{Card, Item, Kind, MALFORMED, UNREAD_FORM, Val, b_tree, emit, head, number_word, val};
use crate::ir::{self, ElemKind as K};

/// A B source's fields read but not appended; `scale` is a voltage's `m`.
#[derive(Default)]
struct Fields {
    tc1: Option<Val>,
    tc2: Option<Val>,
    temp: Option<Val>,
    dtemp: Option<Val>,
    scale: Option<Val>,
}

/// An E, F, G or H card.
pub(super) fn efgh(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item], text: &str) {
    let letter = card.name.as_bytes()[0].to_ascii_lowercase();
    let voltage_ctl = matches!(letter, b'e' | b'g');
    let current_out = matches!(letter, b'g' | b'f');
    if items.len() < 3
        || items[..2]
            .iter()
            .any(|i| i.kind != Kind::Word || i.text == "=")
    {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    card.nodes = vec![items[0].text.clone(), items[1].text.clone()];
    let mut rest = &items[2..];
    // a trailing `m=`: the multiplicity of a G or F (vccs.c, cccs.c), the value's factor on
    // the B source `E … vol=` becomes (run: `vol={v(b)*2} m=3` = 3 V at 0.5 V), an error on
    // POLY (XSPICE poly: "Bad real value", run) and on any other E or H ("unknown parameter")
    let mut m = None;
    if let Some((last, init)) = rest.split_last()
        && last.kind == Kind::Assign
        && last.text.eq_ignore_ascii_case("m")
    {
        m = Some(match val(lw, st, &last.value) {
            Ok(v) => v,
            Err(e) => return lw.opaque(st, e.class, e.code),
        });
        rest = init;
    }
    let Some(first) = rest.first() else {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    };
    let word = first.text.to_ascii_lowercase();
    if first.kind == Kind::Assign && behav_key(letter, &word) {
        // `max=` clamps the value from above (settle e_max.sp: `vol='v(x)*5' max=7` is 7 at
        // 10; ESPice only): read as `min(value, max)`, sugar the writer spells as a call
        let clamp = rest.get(1).filter(|i| {
            i.kind == Kind::Assign && i.text.eq_ignore_ascii_case("max") && rest.len() == 2
        });
        if rest.len() != 1 && clamp.is_none() {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        let expr = match clamp {
            Some(max) => {
                let inner = |t: &str| match unwrap(t) {
                    Wrap::Whole(i, false) | Wrap::Bare(i) => Some(i.to_string()),
                    _ => None,
                };
                let (Some(e), Some(m)) = (inner(&first.value), inner(&max.value)) else {
                    return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
                };
                format!("min(({e}), ({m}))")
            }
            None => first.value.clone(),
        };
        let mut f = Fields::default();
        let output = if current_out {
            card.mult = m;
            ir::controlled::OutKind::Current
        } else {
            f.scale = m;
            ir::controlled::OutKind::Voltage
        };
        return behav(lw, st, &card, &expr, output, f);
    }
    let word0 = first.text.to_ascii_lowercase();
    let keyword = ["table", "vcvs", "vccs", "ccvs", "cccs"].contains(&word0.as_str());
    let poly_form =
        word0.starts_with("poly(") || (!keyword && rest.len() > if voltage_ctl { 3 } else { 2 });
    if first.kind != Kind::Word || m.is_some() && (!current_out || poly_form) {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    card.mult = m;
    if word.starts_with("laplace") {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    if word == "table" && voltage_ctl {
        return table(lw, st, &card, &rest[1..], current_out);
    }
    if let Some(n) = word.strip_prefix("poly(").and_then(|w| w.strip_suffix(')')) {
        let Ok(n) = n.trim().parse::<u8>() else {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        };
        return poly(lw, st, &card, &rest[1..], n, letter);
    }
    if text.to_ascii_lowercase().contains("poly") || word == "table" {
        return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
    }
    let hspice = if voltage_ctl {
        ["vcvs", "vccs"]
    } else {
        ["ccvs", "cccs"]
    };
    if hspice.contains(&word.as_str()) {
        rest = &rest[1..];
    }
    let controls = if voltage_ctl { 2 } else { 1 };
    let ctl_ok = rest.len() > controls && rest[..controls].iter().all(|i| i.kind == Kind::Word);
    if !ctl_ok {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    if rest.len() > controls + 1 {
        // spice2g6: more than one coefficient after one control is POLY(1)
        return poly(lw, st, &card, rest, 1, letter);
    }
    let gain_text = match rest[controls].kind {
        Kind::Assign if rest[controls].text.eq_ignore_ascii_case("gain") => {
            rest[controls].value.clone()
        }
        Kind::Word | Kind::Brace => rest[controls].as_value(),
        Kind::Assign | Kind::Group => {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
    };
    let gain = match val(lw, st, &gain_text) {
        Ok(v) => v,
        Err(e) => return lw.opaque(st, e.class, e.code),
    };
    let kind = match letter {
        b'e' => K::Vcvs,
        b'g' => K::Vccs,
        b'f' => K::Cccs,
        _ => K::Ccvs,
    };
    if voltage_ctl {
        card.nodes.extend(rest[..2].iter().map(|i| i.text.clone()));
    }
    let h = head(lw, st, &card);
    let gain = emit(lw, st, &gain);
    let sense = if voltage_ctl {
        ir::NameId::NONE
    } else {
        let s = lw.names.fold(&rest[0].text);
        lw.b.name(&s)
    };
    let span = lw.span(st.node);
    let row = ir::controlled::Controlled { gain, sense };
    lw.b.controlled(st.scope, kind, h, row, span);
}

// ---- private ----

/// Whether `key=` after the nodes makes the card a B source (`inp_compat`).
fn behav_key(letter: u8, key: &str) -> bool {
    match letter {
        b'e' => matches!(key, "value" | "vol"),
        b'g' => matches!(key, "value" | "cur"),
        _ => false,
    }
}

/// Pushes a `Behav` of `expr` (B grammar).
fn behav(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    card: &Card,
    expr: &str,
    output: ir::controlled::OutKind,
    f: Fields,
) {
    let tree = match b_tree(expr) {
        Ok(t) => t,
        Err(e) => return lw.opaque(st, e.class, e.code),
    };
    let h = head(lw, st, card);
    let mut put = |v: &Option<Val>| v.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let scale = put(&f.scale);
    let mut expr = lw.expr(st, &tree, st.scope);
    if !scale.is_none() {
        // a voltage's `m` multiplies its value (asrcload.c:49-52; R31)
        expr = lw.b.binary(ir::BinOp::Mul, scale, expr);
    }
    let mut put = |v: &Option<Val>| v.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let (tc1, tc2) = (put(&f.tc1), put(&f.tc2));
    let (temp_abs, dtemp) = (put(&f.temp), put(&f.dtemp));
    let tc_base = if tc1.is_none() && tc2.is_none() {
        ir::passive::TcBase::Tnom
    } else {
        ir::passive::TcBase::Fixed27
    };
    let row = ir::controlled::Behav {
        expr,
        tc1,
        tc2,
        temp_abs,
        dtemp,
        rpar: ir::ExprId::NONE,
        output,
        tc_base,
        tc_law: ir::passive::TcLaw::Direct,
    };
    let span = lw.span(st.node);
    lw.b.behav(st.scope, K::Behav, h, row, span);
}

/// `POLY(n)` controls and coefficients after it.
fn poly(lw: &mut Lower<'_, '_>, st: Stmt, card: &Card, rest: &[Item], n: u8, letter: u8) {
    let voltage_ctl = matches!(letter, b'e' | b'g');
    let names = usize::from(n) * if voltage_ctl { 2 } else { 1 };
    let ok = n >= 1
        && rest.len() > names
        && rest[..names].iter().all(|i| i.kind == Kind::Word)
        && rest[names..]
            .iter()
            .all(|i| i.kind == Kind::Brace || number_word(&i.text).is_some());
    if !ok {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    let mut coeffs = Vec::new();
    for i in &rest[names..] {
        match val(lw, st, &i.as_value()) {
            Ok(v) => coeffs.push(v),
            Err(e) => return lw.opaque(st, e.class, e.code),
        }
    }
    let h = head(lw, st, card);
    let controls: Vec<String> = rest[..names]
        .iter()
        .map(|i| {
            if voltage_ctl {
                lw.names.node(&i.text)
            } else {
                lw.names.fold(&i.text)
            }
        })
        .collect();
    let refs: Vec<&str> = controls.iter().map(String::as_str).collect();
    let controls = lw.b.names(&refs);
    let coeffs: Vec<ir::ExprId> = coeffs.iter().map(|v| emit(lw, st, v)).collect();
    let coeffs = lw.b.exprs(&coeffs);
    let q = |v: bool| {
        if v {
            ir::controlled::Quantity::Voltage
        } else {
            ir::controlled::Quantity::Current
        }
    };
    let row = ir::controlled::Poly {
        controls,
        coeffs,
        dimension: n,
        output: q(matches!(letter, b'e' | b'h')),
        control: q(voltage_ctl),
    };
    let span = lw.span(st.node);
    lw.b.poly(st.scope, h, row, span);
}

/// `table {e} [=] (x, y) …`.
fn table(lw: &mut Lower<'_, '_>, st: Stmt, card: &Card, rest: &[Item], current: bool) {
    let Some((input, points)) = rest.split_first() else {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    };
    let points: Vec<&Item> = points.iter().filter(|i| i.text != "=").collect();
    if input.kind != Kind::Brace || points.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    let tree = match b_tree(&input.as_value()) {
        Ok(t) => t,
        Err(e) => return lw.opaque(st, e.class, e.code),
    };
    let mut xy = Vec::new();
    for p in &points {
        let pair: Vec<&str> = p.text.split([',', ' ']).filter(|s| !s.is_empty()).collect();
        let nums: Option<Vec<_>> = pair.iter().map(|s| number_word(s)).collect();
        match nums {
            Some(n) if p.kind == Kind::Group && n.len() == 2 => xy.extend(n),
            // a braced point is numparam's: a number once substituted (settle
            // table_brace_point.sp), which the reader cannot evaluate; the IR's expression
            // point is one the writer refuses (R74 (4)), so the card stays ngspice's text
            _ => return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM),
        }
    }
    let h = head(lw, st, card);
    let input = lw.expr(st, &tree, st.scope);
    let ids: Vec<ir::ExprId> = xy.into_iter().map(|s| emit(lw, st, &Val::of(s))).collect();
    let points = lw.b.exprs(&ids);
    let output = if current {
        ir::controlled::Quantity::Current
    } else {
        ir::controlled::Quantity::Voltage
    };
    let row = ir::controlled::TableSource {
        input,
        points,
        delta: ir::ExprId::NONE,
        law: ir::controlled::TableLaw::NgspiceRounded,
        output,
    };
    let span = lw.span(st.node);
    lw.b.table_source(st.scope, h, row, span);
}
