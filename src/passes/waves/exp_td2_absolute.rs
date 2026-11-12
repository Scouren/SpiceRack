//! Pass `exp_td2_absolute`: a design in, the same design out with every exponential whose
//! second delay counts from `td1` (`TdFrom::Td1`, VACASK) written with it counted from t = 0
//! (`TdFrom::Zero`, SPICE): `td2` becomes `td1 + td2`. `wave_eval` puts the second edge at
//! `td1 + td2` under `Td1` and at the field's value under `Zero`, and the field is that sum,
//! evaluated the same way: exact, bit for bit. `td1` is then read twice: a `td1` that may hold
//! a draw, or ngspice's zero rule (its own pass first), leaves the wave with note `U0203`.
//!
//! Removes `Circuit(ExpTd2FromTd1)`, requires `Circuit(ExpTd2FromZero)`, `Binary(Add)` in both
//! forms, `Expr(Binary)`. One row.
//!
//! Cites: `rewrite/AMEND-A12.md` C2, A30; `rewrite/DIALECT-VACASK.md` §6.2.
//!
//! Contents: `pub fn exp_td2_absolute(&mut ir::Design, &mut ir::Diags)`

use super::EXP_TD2_LEFT;
use super::nodes::{may_draw, note_left, sources};
use crate::ir;
use crate::ir::wave::{Exp, GivenZero, TdFrom};

/// Counts every exponential's second delay from t = 0; notes one it cannot.
pub fn exp_td2_absolute(design: &mut ir::Design, diags: &mut ir::Diags) {
    let rows: Vec<u32> = sources(design)
        .into_iter()
        .filter(|(_, s)| !s.wave.is_none() && s.wave.kind == ir::WaveKind::Exp)
        .map(|(_, s)| s.wave.index)
        .collect();
    for index in rows {
        let e: Exp = design.detail(index);
        let ok = e.td2_from == TdFrom::Td1 && e.given_zero == GivenZero::AsGiven;
        if !ok || e.td1.is_none() || e.td2.is_none() || may_draw(design, e.td1) {
            continue;
        }
        let mut editor = design.editor();
        let td2 = editor.builder().binary(ir::BinOp::Add, e.td1, e.td2);
        let row = Exp {
            td2,
            td2_from: TdFrom::Zero,
            ..e
        };
        editor.set_detail(index, row);
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::ExpTd2FromTd1);
    note_left(design, diags, feature, EXP_TD2_LEFT);
}
