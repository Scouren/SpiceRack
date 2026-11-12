//! T, O, U, Y and P cards → `LosslessLine`, `LossyLine`, `RcLine`, `SingleLossyLine` and
//! `CoupledLines` rows with their `Line` detail.
//!
//! - `T a+ a- b+ b- z0=v td=v | f=v [nl=v]` (`inp2t.c`, `tra.c`; `zo` is `z0`): `z0` is required
//!   ("transmission line z0 must be given", fatal); without `td` the line is `f` and `nl` with
//!   ngspice's defaults written in (`trasetup.c`: `nl` 0.25, `f` 1e9); the initial conditions and
//!   `rel`/`abs` have no IR field (`Unread`);
//! - `O a+ a- b+ b- model` (`inp2o.c`), the length is the model's;
//! - `U a b c model [l=v] [n=v]` (`inp2u.c`, `urc.c`): `l` is the length, `n` the lumps;
//! - `Y a+ a- b+ b- model [len=v]` (`inp2y.c`);
//! - `P i1 … ik ref1 o1 … ok ref2 model [len=v]` (`inp2p.c`: the conductor count from the words
//!   before `len`/`length`).
//!
//! None takes `m`.
//!
//! Contents: `pub(super) fn line(lw: &mut Lower<'_, '_>, st: Stmt, card: Card, items: &[Item])`

use super::super::read::{Lower, Stmt};
use super::{Card, Item, Kind, MALFORMED, UNREAD_FORM, Val, emit, head, literal, push_done, val};
use crate::ir::{self, ElemKind as K, NumForm};

/// A T, O, U, Y or P card.
pub(super) fn line(lw: &mut Lower<'_, '_>, st: Stmt, mut card: Card, items: &[Item]) {
    let letter = card.name.as_bytes()[0].to_ascii_lowercase();
    let words = items.iter().take_while(|i| i.kind == Kind::Word).count();
    let (kind, nodes) = match letter {
        b't' => (K::LosslessLine, 4),
        b'o' => (K::LossyLine, 4),
        b'u' => (K::RcLine, 3),
        b'y' => (K::SingleLossyLine, 4),
        _ => (K::CoupledLines, words.saturating_sub(1)),
    };
    let model = letter != b't';
    let need = nodes + usize::from(model);
    let even = letter != b'p' || (nodes % 2 == 0 && (4..=18).contains(&nodes));
    if words != need || !even {
        return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
    }
    card.nodes = items[..nodes].iter().map(|i| i.text.clone()).collect();
    if model {
        card.model = Some(items[nodes].text.clone());
    }
    let mut f: [Option<Val>; 5] = Default::default();
    const Z0: usize = 0;
    const TD: usize = 1;
    const FREQ: usize = 2;
    const NL: usize = 3;
    const LEN: usize = 4;
    let mut lumps = None;
    for it in &items[need..] {
        let key = it.text.to_ascii_lowercase();
        let slot = match (letter, key.as_str()) {
            (b't', "z0" | "zo") => Some(Z0),
            (b't', "td") => Some(TD),
            (b't', "f") => Some(FREQ),
            (b't', "nl") => Some(NL),
            (b'u', "l") | (b'y', "len") | (b'p', "len" | "length") => Some(LEN),
            (b'u', "n") => None,
            (b't', "ic" | "v1" | "v2" | "i1" | "i2" | "rel" | "abs") | (b'o', "ic") => {
                return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
            }
            _ => return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED),
        };
        if it.kind != Kind::Assign {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        let v = match val(lw, st, &it.value) {
            Ok(v) => v,
            Err(e) => return lw.opaque(st, e.class, e.code),
        };
        let taken = match slot {
            Some(k) => f[k].replace(v).is_some(),
            None => lumps.replace(v).is_some(),
        };
        if taken {
            return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
        }
    }
    if letter == b't' {
        if f[Z0].is_none() {
            return lw.opaque(st, ir::OpaqueClass::InvalidInSource, MALFORMED);
        }
        if f[TD].is_some() && (f[FREQ].is_some() || f[NL].is_some()) {
            return lw.opaque(st, ir::OpaqueClass::Unread, UNREAD_FORM);
        }
        if f[TD].is_none() {
            // trasetup.c: f 1e9, nl 0.25 when not given
            f[FREQ].get_or_insert(Val::Lit(literal(1, 0, 9, NumForm::Exponent)));
            f[NL].get_or_insert(Val::Lit(literal(25, 2, 0, NumForm::Plain)));
        }
    }
    let h = head(lw, st, &card);
    let mut put = |v: &Option<Val>| v.as_ref().map_or(ir::ExprId::NONE, |v| emit(lw, st, v));
    let (z0, td, freq, nl, length) = (
        put(&f[Z0]),
        put(&f[TD]),
        put(&f[FREQ]),
        put(&f[NL]),
        put(&f[LEN]),
    );
    let lumps = put(&lumps);
    let row = ir::line::Line {
        z0,
        td,
        freq,
        nl,
        length,
        vel: ir::ExprId::NONE,
        lumps,
    };
    let span = lw.span(st.node);
    let item = lw.b.line(st.scope, kind, h, row, span);
    push_done(lw, st, item, &card);
}
