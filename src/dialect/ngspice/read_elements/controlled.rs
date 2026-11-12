//! E, F, G, H and B cards → `Vcvs`, `Vccs`, `Cccs`, `Ccvs`, `Poly`, `Table` and `Behav` rows.
//!
//! The forms (`inp2e.c`… `inp2h.c`, `inp2b.c`, and `inpcom.c` before them):
//! - linear: `E|G n+ n- c+ c- gain`, `F|H n+ n- vname gain`; the HSPICE words `vcvs`, `vccs`,
//!   `cccs`, `ccvs` after the nodes are dropped (`inp_compat` `replace_token`). `m` is a
//!   parameter of G and F only (`vccs.c`, `cccs.c`; run: `E1 a 0 b 0 2 m=3` is "unknown
//!   parameter (m)", `InvalidInSource`), and an error on any POLY form (run: G/F `poly(1) … m=3`
//!   is XSPICE's "Bad real value");
//! - `POLY(n)` controls coefficients (XSPICE `poly`, `enhtrans.c`); more than one number after
//!   one control is `POLY(1)` (`inp_poly_2g6_compat`; run: `E2 f 0 poly(1) b 0 0 2` = 1 V at
//!   0.5 V);
//! - `E … value={e}` / `vol={e}` and `G … value={e}` / `cur={e}` are B sources (`inp_compat`;
//!   runs: 2 V and 3 V), the expression the B-source parser's; `m` multiplies the E's value (run:
//!   3 V for 1 V, read `Mul(m, e)` as on a voltage B, R31) and is the G's multiplicity (run);
//! - `E|G … table {e} = (x, y) …`: XSPICE `pwl` with `input_domain=0.1 fraction=TRUE`, ngspice's
//!   rounded corners (`TableLaw::NgspiceRounded`; run: 1 V at 0.5 V on (0,0) (1,2));
//! - `E … laplace …`: ngspice-45 has none (run: "Undefined parameter [v]", fatal),
//!   `InvalidInSource`;
//! - `B n+ n- v=e | i=e [tc1 tc2 temp dtemp m]`: the expression runs to the next parameter key
//!   (run: `B1 c 0 V=1 + 2 tc1=0` = 3 V); `m` multiplies a current (parallel copies) and a
//!   voltage's value (run: `V=1 m=2` = 2 V; `asrcload.c:49-52`), read as `Mul(m, expr)` (R31,
//!   sugar under R13); `tc1`/`tc2` are measured from 27 °C whatever `tnom` (`asrcload.c:41`:
//!   `TcBase::Fixed27`); `reciproctc`, `reciprocm`, a `=pwl(` B line are ngspice's own
//!   (`Private`).
//!
//! Contents: `pub(super) fn efgh`, `pub(super) fn b` (each
//! `(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item], text: &str)`)

use super::super::read::{Lower, Stmt};
use super::{
    Card, Item, Kind, MALFORMED, PRIVATE_FORM, UNREAD_FORM, Val, b_tree, emit, head, number_word,
    val,
};
use crate::ir::{self, ElemKind as K};

/// The B-source parameter keys that end a `v=`/`i=` expression (`asrc.c`).
const B_KEYS: &[&str] = &[
    "v",
    "i",
    "temp",
    "dtemp",
    "tc1",
    "tc2",
    "m",
    "reciproctc",
    "reciprocm",
];

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
        if rest.len() != 1 {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        let mut f = Fields::default();
        let output = if current_out {
            card.mult = m;
            ir::controlled::OutKind::Current
        } else {
            f.scale = m;
            ir::controlled::OutKind::Voltage
        };
        return behav(lw, st, &card, &first.value, output, f);
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

/// A B card.
pub(super) fn b(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item], text: &str) {
    if items.len() < 3
        || items[..2]
            .iter()
            .any(|i| i.kind != Kind::Word || i.text == "=")
    {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    if text.to_ascii_lowercase().replace(' ', "").contains("=pwl(") {
        return lw.opaque(st, ir::OpaqueClass::Private, PRIVATE_FORM);
    }
    card.nodes = vec![items[0].text.clone(), items[1].text.clone()];
    let key = |i: &Item| i.kind == Kind::Assign && B_KEYS.contains(&lower(&i.text).as_str());
    let first = &items[2];
    let output = match lower(&first.text).as_str() {
        "v" if first.kind == Kind::Assign => ir::controlled::OutKind::Voltage,
        "i" if first.kind == Kind::Assign => ir::controlled::OutKind::Current,
        _ => return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED),
    };
    // the expression: from the value to the next parameter key (inp2b.c, run)
    let end = items[3..]
        .iter()
        .position(key)
        .map_or(items.len(), |p| p + 3);
    let value_at = first.at + text[first.at..].find('=').expect("an assignment") + 1;
    let stop = items.get(end).map_or(text.len(), |i| i.at);
    let expr = text[value_at..stop].trim().to_string();
    let mut f = Fields::default();
    for it in &items[end..] {
        if !key(it) {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        let v = match val(lw, st, &it.value) {
            Ok(v) => v,
            Err(e) => return lw.opaque(st, e.class, e.code),
        };
        let taken = match lower(&it.text).as_str() {
            "temp" => f.temp.replace(v).is_some(),
            "dtemp" => f.dtemp.replace(v).is_some(),
            "m" if output == ir::controlled::OutKind::Voltage => f.scale.replace(v).is_some(),
            "m" => card.mult.replace(v).is_some(),
            "tc1" => f.tc1.replace(v).is_some(),
            "tc2" => f.tc2.replace(v).is_some(),
            "reciproctc" | "reciprocm" => {
                return lw.opaque(st, ir::OpaqueClass::Private, PRIVATE_FORM);
            }
            // a second `v=`/`i=`
            _ => return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED),
        };
        card.dups |= taken;
    }
    behav(lw, st, &card, &expr, output, f);
}

// ---- private ----

/// The ASCII lowercase of a key.
fn lower(text: &str) -> String {
    text.to_ascii_lowercase()
}

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
            // an expression in a table point: XSPICE pwl reads numbers only (inpcom.c FIXME)
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
