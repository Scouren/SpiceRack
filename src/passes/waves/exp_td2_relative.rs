//! Pass `exp_td2_relative`: a design in, the same design out with every exponential whose
//! second delay counts from t = 0 (`TdFrom::Zero`, SPICE) written with it counted from `td1`
//! (`TdFrom::Td1`, VACASK): `td2` becomes the literal of `td2 − td1`. `wave_eval` puts the
//! second edge at `td2` under `Zero` and at `td1 + td2'` under `Td1`, so the rule is exact
//! exactly when `td1 + (td2 − td1)` is `td2` again in doubles; the pass checks that on the two
//! literals (and `td2 > td1`: VACASK requires a positive second delay), and writes
//! `Literal::from_f64` of the difference, which reads back as that double. A field that is no
//! literal, ngspice's zero rule (its own pass first), `td2 ≤ td1`, or a difference that does
//! not round back leaves the wave with note `U0203`.
//!
//! Removes `Circuit(ExpTd2FromZero)`, requires `Circuit(ExpTd2FromTd1)`, `Expr(Num)`. One row.
//!
//! Cites: `rewrite/AMEND-A12.md` C2, A30; `rewrite/DIALECT-VACASK.md` §6.1.
//!
//! Contents: `pub fn exp_td2_relative(&mut ir::Design, &mut ir::Diags)`

use super::EXP_TD2_LEFT;
use super::nodes::{literal, note_left, sources};
use crate::ir;
use crate::ir::wave::{Exp, GivenZero, TdFrom};

/// Counts every exponential's second delay from `td1` where that is exact; notes the rest.
pub fn exp_td2_relative(design: &mut ir::Design, diags: &mut ir::Diags) {
    let rows: Vec<u32> = sources(design)
        .into_iter()
        .filter(|(_, s)| !s.wave.is_none() && s.wave.kind == ir::WaveKind::Exp)
        .map(|(_, s)| s.wave.index)
        .collect();
    for index in rows {
        let e: Exp = design.detail(index);
        if e.td2_from != TdFrom::Zero || e.given_zero != GivenZero::AsGiven {
            continue;
        }
        let (Some(td1), Some(td2)) = (literal(design, e.td1), literal(design, e.td2)) else {
            continue;
        };
        let gap = td2 - td1;
        if td2 <= td1 || td1 + gap != td2 || !gap.is_finite() {
            continue;
        }
        let mut editor = design.editor();
        let td2 = editor.builder().literal(ir::Literal::from_f64(gap));
        let row = Exp {
            td2,
            td2_from: TdFrom::Td1,
            ..e
        };
        editor.set_detail(index, row);
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::ExpTd2FromZero);
    note_left(design, diags, feature, EXP_TD2_LEFT);
}
