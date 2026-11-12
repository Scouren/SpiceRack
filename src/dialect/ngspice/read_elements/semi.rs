//! D, Q, M, J, Z cards → `Diode`, `Bjt`, `Mosfet`, `Jfet`, `Mesfet` rows with their `Semi`
//! detail; S and W cards → `VSwitch`, `ISwitch` with their `Switch` detail.
//!
//! Which word is the model is ngspice's frontend rule (`inpcom.c` `get_number_terminals`,
//! which picks the model card a line keeps: a wrong guess drops the model, "could not find a
//! valid modelname"), not `inp2d.c`'s model lookup: D's nodes are the items before the first
//! `off`, `thermal` or `k=v` from the fourth on, less one (so `D1 a k dmod 2` takes `2` as its
//! model: run, `InvalidInSource`; `D1 a k t dmod` has the thermal node); M's the same from the
//! fifth (`off`, `tnodeout`, `thermal`, `k=v`); Q's are the words before its flags and
//! parameters less the model, and less a digits-only area right after it; J and Z have 3 nodes
//! and the model fourth. An unlabeled number after the model is the area (Q, J; M rejects it:
//! "no unlabeled parameter permitted on mosfet"; on Z it depends on the model, `Unread`). `off`
//! is the first guess; `ic=a,b,…` the device order (applied only under `uic`, INV 8.21:
//! `IcMode::UicOnly`); `area=`, `temp=`, `dtemp=` are the fields (`dio.c`, `bjt.c`, `jfet.c`,
//! `mes.c` area; every MOS, HFET and HiSIM that takes `temp`/`dtemp` gives it the instance
//! temperature: run mm, MOS1 `temp=50`; BSIM4 and MES have none, "unknown parameter", a fact of
//! the bound model the reader does not know); `m` is the multiplicity; a key given twice keeps
//! the last (run: `temp=50 temp=60` is 60); `area` on M and the IR's own names `mult`,
//! `temp_abs`, `ic_mode`, `initial` are no ngspice key ("unknown parameter", runs mm:
//! `InvalidInSource`; A19 keeps a typed field's name out of the parameters); every other `k=v` is
//! an instance parameter by its own name.
//!
//! `S n+ n- c+ c- model [on|off]` (`inp2s.c`: the fifth token is the model; an unlabeled number
//! is ignored, `Unread`); `W n+ n- vctl model [on|off]` (`inp2w.c`). Neither takes `m`.
//!
//! Contents: `pub(super) fn semi`, `pub(super) fn switch` (each
//! `(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item])`)

use super::super::read::{Lower, Stmt};
use super::{
    Card, Item, Kind, MALFORMED, UNREAD_FORM, Val, emit, head, number_word, push_done, val,
};
use crate::ir::{self, ElemKind as K};

/// A D, Q, M, J or Z card.
pub(super) fn semi(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item]) {
    let letter = card.name.as_bytes()[0].to_ascii_lowercase();
    let (kind, min, max) = match letter {
        b'd' => (K::Diode, 2, 3),
        b'q' => (K::Bjt, 3, 5),
        b'm' => (K::Mosfet, 3, 7),
        b'j' => (K::Jfet, 3, 3),
        _ => (K::Mesfet, 3, 3),
    };
    let in_card = |k: &usize| *k < items.len() && (min..=max).contains(k);
    let Some(at) = model_at(letter, items).filter(in_card) else {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    };
    let model_ok = items[at].kind == Kind::Word && number_word(&items[at].text).is_none();
    if !model_ok
        || items[..at]
            .iter()
            .any(|i| i.kind != Kind::Word || i.text == "=")
    {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    card.nodes = items[..at].iter().map(|i| i.text.clone()).collect();
    card.model = Some(items[at].text.clone());
    let mut rest = &items[at + 1..];
    let (mut area, mut temp, mut dtemp) = (None, None, None);
    let mut ic: Vec<Val> = Vec::new();
    let mut initial = ir::semi::InitialState::Unset;
    if let Some(first) = rest
        .first()
        .filter(|i| i.kind != Kind::Assign && !flag(&i.text))
    {
        if letter == b'z' {
            return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
        }
        if letter == b'm' || first.kind != Kind::Word && first.kind != Kind::Brace {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        area = Some(match val(lw, st, &first.as_value()) {
            Ok(v) => v,
            Err(e) => return lw.opaque(st, e.class, e.code),
        });
        rest = &rest[1..];
    }
    let mut k = 0;
    // Bounded: each turn takes at least one item.
    while k < rest.len() {
        let it = &rest[k];
        k += 1;
        let key = it.text.to_ascii_lowercase();
        if it.kind == Kind::Word && key == "off" {
            initial = ir::semi::InitialState::Off;
            continue;
        }
        if it.kind != Kind::Assign {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        let mut texts = vec![it.value.clone()];
        if key == "ic" {
            // `ic=a,b,c`: the values after the first stand behind commas
            while let Some(n) = rest.get(k).filter(|n| n.comma && n.kind != Kind::Assign) {
                texts.push(n.as_value());
                k += 1;
            }
        }
        let mut vals = Vec::new();
        for t in &texts {
            match val(lw, st, t) {
                Ok(v) => vals.push(v),
                Err(e) => return lw.opaque(st, e.class, e.code),
            }
        }
        let one = || vals[0].clone();
        let taken = match key.as_str() {
            "ic" => !std::mem::replace(&mut ic, vals.clone()).is_empty(),
            "m" => card.mult.replace(one()).is_some(),
            "area" if letter != b'm' => area.replace(one()).is_some(),
            "temp" => temp.replace(one()).is_some(),
            "dtemp" => dtemp.replace(one()).is_some(),
            // no ngspice device has these keys ("unknown parameter", runs mm), and a
            // parameter may not bear a typed field's name (A19): no MOS takes `area`
            "area" | "mult" | "temp_abs" | "ic_mode" | "initial" => {
                return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
            }
            _ => {
                card.param(&it.text, one());
                false
            }
        };
        card.dups |= taken;
    }
    let h = head(lw, st, &card);
    let area = area.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let ics: Vec<ir::ExprId> = ic.iter().map(|v| emit(lw, st, v)).collect();
    let ic = if ics.is_empty() {
        ir::List::EMPTY
    } else {
        lw.b.exprs(&ics)
    };
    let temp_abs = temp.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let dtemp = dtemp.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let ic_mode = if ic.is_empty() {
        ir::passive::IcMode::None
    } else {
        ir::passive::IcMode::UicOnly
    };
    let row = ir::semi::Semi {
        area,
        ic,
        temp_abs,
        dtemp,
        ic_mode,
        initial,
    };
    let span = lw.span(st.node);
    let item = lw.b.semi(st.scope, kind, h, row, span);
    push_done(lw, st, item, &card);
}

/// An S or W card.
pub(super) fn switch(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item]) {
    let current = card.name.as_bytes()[0].eq_ignore_ascii_case(&b'w');
    let (n, kind) = if current {
        (3, K::ISwitch)
    } else {
        (4, K::VSwitch)
    };
    let words = items.len() > n && items[..=n].iter().all(|i| i.kind == Kind::Word);
    if !words {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    let mut initial = ir::semi::InitialState::Unset;
    for it in &items[n + 1..] {
        initial = match it.text.to_ascii_lowercase().as_str() {
            "on" if it.kind == Kind::Word => ir::semi::InitialState::On,
            "off" if it.kind == Kind::Word => ir::semi::InitialState::Off,
            _ if number_word(&it.text).is_some() => {
                // an unlabeled number: inp2s.c ignores it
                return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
            }
            _ => return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED),
        };
    }
    let nodes = if current { 2 } else { 4 };
    card.nodes = items[..nodes].iter().map(|i| i.text.clone()).collect();
    card.model = Some(items[n].text.clone());
    let h = head(lw, st, &card);
    let sense = if current {
        let s = lw.names.fold(&items[2].text);
        lw.b.name(&s)
    } else {
        ir::NameId::NONE
    };
    let row = ir::semi::Switch {
        sense,
        control: ir::ExprId::NONE,
        initial,
    };
    let span = lw.span(st.node);
    let item = lw.b.switch(st.scope, kind, h, row, span);
    push_done(lw, st, item, &card);
}

// ---- private ----

/// The index of the model item by `get_number_terminals` (`None` when the card has none).
fn model_at(letter: u8, items: &[Item]) -> Option<usize> {
    let flagged = |from: usize, words: &[&str]| {
        let k = items.iter().enumerate().skip(from).position(|(_, i)| {
            i.kind == Kind::Assign || words.iter().any(|w| i.text.eq_ignore_ascii_case(w))
        });
        k.map_or(items.len(), |k| k + from)
    };
    match letter {
        b'd' => flagged(3, &["off", "thermal"]).checked_sub(1),
        b'm' => flagged(4, &["off", "tnodeout", "thermal"]).checked_sub(1),
        b'q' => {
            let bare = items
                .iter()
                .take_while(|i| i.kind != Kind::Assign && !flag(&i.text) && !i.comma)
                .count();
            let last = &items.get(bare.checked_sub(1)?)?.text;
            let area = !last.bytes().any(|c| c.is_ascii_alphabetic() || c == b',');
            bare.checked_sub(if area { 2 } else { 1 })
        }
        _ => Some(3),
    }
}

/// A word that is an instance flag, never a node or model (`off`, `on`).
fn flag(word: &str) -> bool {
    word.eq_ignore_ascii_case("off") || word.eq_ignore_ascii_case("on")
}
