//! R, C, L and K cards → `Resistor`, `Capacitor`, `Inductor`, `ChargeCapacitor` and `Coupling`
//! rows.
//!
//! `R|C|L name n1 n2 [value] [model] [value] k=v …` (`inp2r.c`, `inp2c.c`, `inp2l.c`): the value
//! is a number or `{…}` right after the nodes, or the "unlabeled number" after the model
//! (`waslead`; runs: `R4 c 0 rmod 2k`, `L1 a 0 lmod 1u` read 2k and 1u), or `r=`/`c=`/`l=` and
//! the device's own names; a bare word after the value is the model (`INPlookMod`, else an
//! error). `tc=a b` is `tc1=a tc2=b` (`inp2r.c`). An R with neither value nor model is 1 mΩ
//! (`restemp.c:79`, run: "set to 1 mOhm"); `R1 a 0` with nothing after its nodes is ignored by
//! ngspice (run: "not a valid resistor instance line, ignored"), `InvalidInSource`.
//!
//! An R/C/L whose card after the nodes holds `v(`, `i(`, `temper`, `hertz` or `time`
//! (`inpcom.c` `b_transformation_wanted`), or a C with `q=`, is run as a B source built from the
//! first `{…}` on the line (`inp_compat`, INV 6.14, 14.13): that expression is the value (B
//! grammar), `m` stays, the noise switch stays on an R (`noisy=1`), every other parameter is
//! dropped (note `DROPPED`); `tc1`/`tc2` are measured from 27 °C whatever `tnom`
//! (`asrcload.c:41`: `TcBase::Fixed27`), directly on an R (`reciproctc=1` on a current is the
//! resistance's factor) and an L, reciprocally on a C (`reciproctc=1`: C/(1+tc1·ΔT+…),
//! `TcLaw::Reciprocal`; R31); on a value that reads only the temperature, on a charge, or a
//! number `eval_tc` rounds to 9 digits they are `Unread` (`TC_BASE`); a first `{…}` that is not
//! the value is `Private` (`FIRST_BRACE`). A C with `q=` is a `ChargeCapacitor`.
//!
//! `K name L1 L2 … k` (`inp2k.c`, `inp_compat`: three or more inductors couple every pair).
//!
//! Contents: `pub(super) fn rcl`, `pub(super) fn coupling` (each
//! `(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item], …)`)

use super::super::read::{Lower, Stmt};
use super::{
    Card, DROPPED, FIRST_BRACE, Item, Kind, MALFORMED, TC_BASE, UNREAD_FORM, Val, b_tree, emit,
    head, literal, number_word, push_done, val,
};
use crate::ir::{self, ElemKind as K, NumForm};

/// What a parameter key of an R, C or L sets.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Slot {
    Value,
    Ic,
    Tc1,
    Tc2,
    Tce,
    Temp,
    Dtemp,
    Ac,
    Scale,
    Mult,
    Noisy,
    Param,
}

/// The typed fields of an R, C or L read but not appended, in `Passive` order.
#[derive(Default)]
struct Fields {
    value: Option<Val>,
    ic: Option<Val>,
    tc1: Option<Val>,
    tc2: Option<Val>,
    tce: Option<Val>,
    temp: Option<Val>,
    dtemp: Option<Val>,
    ac: Option<Val>,
    scale: Option<Val>,
    noisy: bool,
}

/// An `R`, `C` or `L` card.
pub(super) fn rcl(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item], text: &str) {
    let letter = card.name.as_bytes()[0].to_ascii_lowercase();
    let nodes = items.len() >= 3 && items[..2].iter().all(|i| i.kind == Kind::Word);
    if !nodes || items[..2].iter().any(|i| i.text == "=") {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    card.nodes = vec![items[0].text.clone(), items[1].text.clone()];
    let charge = letter == b'c' && text.contains("q=");
    if charge || wanted(&text[items[2].at..]) {
        return dynamic(lw, st, card, letter, &items[2..], charge);
    }
    let mut f = Fields {
        noisy: true,
        ..Fields::default()
    };
    let rest = &items[2..];
    let mut at = 0;
    if is_value(&rest[0]) {
        f.value = Some(match val(lw, st, &rest[0].as_value()) {
            Ok(v) => v,
            Err(e) => return lw.opaque(st, e.class, e.code),
        });
        at = 1;
    }
    if let Some(m) = rest
        .get(at)
        .filter(|i| i.kind == Kind::Word && !is_value(i))
    {
        card.model = Some(m.text.clone());
        at += 1;
        if f.value.is_none() && rest.get(at).is_some_and(is_value) {
            f.value = Some(match val(lw, st, &rest[at].as_value()) {
                Ok(v) => v,
                Err(e) => return lw.opaque(st, e.class, e.code),
            });
            at += 1;
        }
    }
    let mut k = at;
    // Bounded: each turn takes one item.
    while k < rest.len() {
        let it = &rest[k];
        k += 1;
        let Some(slot) = (it.kind == Kind::Assign)
            .then(|| slot(letter, &it.text.to_ascii_lowercase()))
            .flatten()
        else {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        };
        let v = match val(lw, st, &it.value) {
            Ok(v) => v,
            Err(e) => return lw.opaque(st, e.class, e.code),
        };
        // `tc=a b`: the second number is tc2 (inp2r.c)
        let tc_pair = letter == b'r' && it.text.eq_ignore_ascii_case("tc");
        if tc_pair && let Some(next) = rest.get(k).filter(|n| n.kind == Kind::Word) {
            let Some(s) = number_word(&next.text) else {
                return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
            };
            if f.tc2.replace(Val::Lit(s)).is_some() {
                return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
            }
            k += 1;
        }
        let taken = match slot {
            Slot::Value => f.value.replace(v).is_some(),
            Slot::Ic => f.ic.replace(v).is_some(),
            Slot::Tc1 => f.tc1.replace(v).is_some(),
            Slot::Tc2 => f.tc2.replace(v).is_some(),
            Slot::Tce => f.tce.replace(v).is_some(),
            Slot::Temp => f.temp.replace(v).is_some(),
            Slot::Dtemp => f.dtemp.replace(v).is_some(),
            Slot::Ac => f.ac.replace(v).is_some(),
            Slot::Scale => f.scale.replace(v).is_some(),
            Slot::Mult => card.mult.replace(v).is_some(),
            Slot::Noisy => match v {
                Val::Lit(s) if s.literal.frac == 0 && s.literal.exp10 == 0 => {
                    f.noisy = s.literal.mantissa != 0;
                    false
                }
                _ => true,
            },
            Slot::Param => {
                card.params.push((it.text.clone(), v));
                false
            }
        };
        if taken {
            // a second value for one field: ngspice's last one wins; no rule yet
            return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
        }
    }
    if f.value.is_none() && card.model.is_none() {
        if letter != b'r' {
            return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
        }
        // restemp.c:79: no value, no model resistance: 1 mOhm
        f.value = Some(Val::Lit(literal(1, 0, -3, NumForm::Scale)));
    }
    let kind = match letter {
        b'r' => K::Resistor,
        b'c' => K::Capacitor,
        _ => K::Inductor,
    };
    let h = head(lw, st, &card);
    let mut put = |v: &Option<Val>| v.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let value = put(&f.value);
    let ic = put(&f.ic);
    let (tc1, tc2, tce) = (put(&f.tc1), put(&f.tc2), put(&f.tce));
    let (temp_abs, dtemp) = (put(&f.temp), put(&f.dtemp));
    let (ac, scale) = (put(&f.ac), put(&f.scale));
    let ic_mode = if ic.is_none() {
        ir::passive::IcMode::None
    } else {
        ir::passive::IcMode::UicOnly
    };
    let row = ir::passive::Passive {
        value,
        ic,
        tc1,
        tc2,
        tce,
        temp_abs,
        dtemp,
        ac,
        scale,
        ic_mode,
        noisy: f.noisy,
        ..ir::passive::Passive::PLAIN
    };
    let span = lw.span(st.node);
    let item = lw.b.passive(st.scope, kind, h, row, span);
    push_done(lw, st, item, &card);
}

/// A `K` card: two or more inductors and the coefficient.
pub(super) fn coupling(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item]) {
    let Some((last, names)) = items.split_last() else {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    };
    let words = names.iter().all(|i| i.kind == Kind::Word && !is_value(i));
    let k_text = match last.kind {
        Kind::Assign if ["k", "coefficient"].contains(&last.text.to_ascii_lowercase().as_str()) => {
            last.value.clone()
        }
        _ if is_value(last) => last.as_value(),
        _ => String::new(),
    };
    if names.len() < 2 || !words || k_text.is_empty() {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    let k = match val(lw, st, &k_text) {
        Ok(v) => v,
        Err(e) => return lw.opaque(st, e.class, e.code),
    };
    let h = head(lw, st, &card);
    let folded: Vec<String> = names.iter().map(|i| lw.names.fold(&i.text)).collect();
    let refs: Vec<&str> = folded.iter().map(String::as_str).collect();
    let inductors = lw.b.names(&refs);
    let k = emit(lw, st, &k);
    let span = lw.span(st.node);
    let row = ir::passive::Coupling { inductors, k };
    lw.b.coupling(st.scope, h, row, span);
}

// ---- private ----

/// Whether the item is a positional value: a number or a braced expression.
fn is_value(i: &Item) -> bool {
    match i.kind {
        Kind::Brace => true,
        Kind::Word => number_word(&i.text).is_some(),
        Kind::Assign | Kind::Group => false,
    }
}

/// The field a parameter key sets on an R (`res.c`), C (`cap.c`) or L (`ind.c`), or `None` for
/// a key the device does not take.
fn slot(letter: u8, key: &str) -> Option<Slot> {
    let common = match key {
        "temp" => Some(Slot::Temp),
        "dtemp" => Some(Slot::Dtemp),
        "m" => Some(Slot::Mult),
        "tc1" => Some(Slot::Tc1),
        "tc2" => Some(Slot::Tc2),
        "scale" => Some(Slot::Scale),
        _ => None,
    };
    common.or(match (letter, key) {
        (b'r', "resistance" | "r") => Some(Slot::Value),
        (b'r', "tc") => Some(Slot::Tc1),
        (b'r', "tce") => Some(Slot::Tce),
        (b'r', "ac") => Some(Slot::Ac),
        (b'r', "noisy" | "noise") => Some(Slot::Noisy),
        (b'r' | b'c', "l" | "w" | "bv_max") => Some(Slot::Param),
        (b'c', "capacitance" | "cap" | "c") => Some(Slot::Value),
        (b'c' | b'l', "ic") => Some(Slot::Ic),
        (b'l', "inductance" | "l") => Some(Slot::Value),
        (b'l', "nt") => Some(Slot::Param),
        _ => None,
    })
}

/// `inpcom.c` `b_transformation_wanted`: whether the text names `v(`, `i(`, `temper`, `hertz`
/// or `time` at the start of an identifier.
fn wanted(text: &str) -> bool {
    let b = text.as_bytes();
    let ident = |c: u8| c == b'_' || c.is_ascii_alphanumeric();
    (0..b.len()).any(|k| {
        if k > 0 && ident(b[k - 1]) {
            return false;
        }
        let rest = &b[k..];
        let word = |w: &[u8]| rest.starts_with(w) && !rest.get(w.len()).is_some_and(|c| ident(*c));
        rest.starts_with(b"v(")
            || rest.starts_with(b"i(")
            || word(b"temper")
            || word(b"hertz")
            || word(b"time")
    })
}

/// Whether a value text reads the circuit (`v(`, `i(`, `time`, `hertz`): what
/// `TcBase::Fixed27` needs (a value of the temperature alone is no circuit read).
fn reads_circuit(text: &str) -> bool {
    wanted(&text.replace("temper", "_"))
}

/// Whether `eval_tc`'s `%15.8e` changes a number (more than 9 significant digits).
fn rounds(v: &Val) -> bool {
    match v {
        Val::Lit(s) => {
            let mut m = s.literal.mantissa.unsigned_abs();
            // Bounded: at most 19 decimal digits.
            while m != 0 && m.is_multiple_of(10) {
                m /= 10;
            }
            m >= 1_000_000_000
        }
        Val::Tree(_) => false,
    }
}

/// An R, C or L ngspice runs as a B source (`inp_compat`).
fn dynamic(
    lw: &mut Lower<'_, '_>,
    st: Stmt,
    mut card: Card,
    letter: u8,
    rest: &[Item],
    charge: bool,
) {
    let braces = |i: &Item| {
        i.kind == Kind::Brace || (i.kind == Kind::Assign && i.value.starts_with(['{', '\'']))
    };
    let Some(first) = rest.iter().position(braces) else {
        // "mal formed … instance": ngspice exits
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    };
    let is_value_key = |i: &Item| {
        let key = i.text.to_ascii_lowercase();
        i.kind == Kind::Assign
            && if charge {
                key == "q"
            } else {
                slot(letter, &key) == Some(Slot::Value)
            }
    };
    let it = &rest[first];
    if !(is_value_key(it) || it.kind == Kind::Brace && !charge && first == 0) {
        return lw.opaque(st, ir::OpaqueClass::Private, FIRST_BRACE);
    }
    let text = if it.kind == Kind::Brace {
        it.as_value()
    } else {
        it.value.clone()
    };
    let tree = match b_tree(&text) {
        Ok(t) => t,
        Err(e) => return lw.opaque(st, e.class, e.code),
    };
    let mut dropped = false;
    let mut noisy = false;
    let (mut tc1, mut tc2) = (None, None);
    for (k, i) in rest.iter().enumerate() {
        let key = i.text.to_ascii_lowercase();
        if k == first {
            continue;
        }
        match (i.kind, key.as_str()) {
            (Kind::Assign, "tc1" | "tc2") => {
                let v = match val(lw, st, &i.value) {
                    Ok(v) => v,
                    Err(e) => return lw.opaque(st, e.class, e.code),
                };
                let circuit = reads_circuit(&text);
                if charge || !circuit || rounds(&v) {
                    return lw.opaque(st, ir::OpaqueClass::Unread, TC_BASE);
                }
                let slot = if key == "tc1" { &mut tc1 } else { &mut tc2 };
                if slot.replace(v).is_some() {
                    return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
                }
            }
            (Kind::Assign, "m") => match val(lw, st, &i.value) {
                Ok(v) => card.mult = Some(v),
                Err(e) => return lw.opaque(st, e.class, e.code),
            },
            // inpcom.c: strstr "noisy=1"/"noise=1" on an R
            (Kind::Assign, "noisy" | "noise") if letter == b'r' => noisy = i.value == "1",
            _ => dropped = true,
        }
    }
    if dropped {
        lw.note(st, DROPPED);
    }
    let h = head(lw, st, &card);
    let e = lw.expr(st, &tree, st.scope);
    let span = lw.span(st.node);
    if charge {
        let row = ir::controlled::Behav {
            expr: e,
            tc1: ir::ExprId::NONE,
            tc2: ir::ExprId::NONE,
            temp_abs: ir::ExprId::NONE,
            dtemp: ir::ExprId::NONE,
            rpar: ir::ExprId::NONE,
            output: ir::controlled::OutKind::Charge,
            tc_base: ir::passive::TcBase::Tnom,
        };
        lw.b.behav(st.scope, K::ChargeCapacitor, h, row, span);
        return;
    }
    let kind = match letter {
        b'r' => K::Resistor,
        b'c' => K::Capacitor,
        _ => K::Inductor,
    };
    let mut put = |v: &Option<Val>| v.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let (tc1, tc2) = (put(&tc1), put(&tc2));
    let tc = !tc1.is_none() || !tc2.is_none();
    let row = ir::passive::Passive {
        value: e,
        tc1,
        tc2,
        noisy,
        tc_base: if tc {
            ir::passive::TcBase::Fixed27
        } else {
            ir::passive::TcBase::Tnom
        },
        tc_law: if tc && kind == K::Capacitor {
            ir::passive::TcLaw::Reciprocal
        } else {
            ir::passive::TcLaw::Direct
        },
        ..ir::passive::Passive::PLAIN
    };
    lw.b.passive(st.scope, kind, h, row, span);
}
