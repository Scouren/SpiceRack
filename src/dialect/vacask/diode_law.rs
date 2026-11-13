//! Where `sp_diode` and the IR's diode family part (R91): the reader keeps such a statement as
//! VACASK's text and the writer refuses what VACASK would compute otherwise. (A parameter only
//! one side has, `ikp` or `rth0`, is kept as given: each writer refuses what its device lacks,
//! R91a.)
//!
//! - F23, a zero area (settle diode_area_zero, run115): ngspice-45 takes `area=0` as no device
//!   (-5e-13 A at 0.5 V, the gmin current), VACASK warns "area must be positive, using 1 instead"
//!   (-2.4856e-06 A, as at area 1). A literal zero diode area is refused both ways. (A BJT at
//!   area 0 fails to converge on both simulators: no number to differ, run115.)
//!
//! Contents: `ZERO_AREA`, `WRITE_ZERO_AREA`, `CODES`, `zero_area`, `admit` (crate-private)

use super::read::Lower;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// An `sp_diode` instance whose area is a literal zero.
pub(super) const ZERO_AREA: ir::Code = ir::Code::new(b'P', 2110);
/// A diode whose area is a literal zero.
pub(super) const WRITE_ZERO_AREA: ir::Code = ir::Code::new(b'W', 2110);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        ZERO_AREA,
        ir::Info {
            title: "an sp_diode instance with a zero area, kept as its text",
            cite: "R91 (F23); settle diode_area_zero (run115)",
            explain: "VACASK replaces a zero diode area by 1 with a warning; ngspice-45 takes it as no device; the IR's area 0 means ngspice's, so the instance is kept as VACASK's text.",
            kind: ir::Kind::Note,
        },
    ),
    (
        WRITE_ZERO_AREA,
        ir::Info {
            title: "a diode with a zero area",
            cite: "R91 (F23); settle diode_area_zero (run115)",
            explain: "VACASK replaces a zero diode area by 1 (\"area must be positive, using 1 instead\"), where the IR's zero area is no device; VACASK has no spelling of it.",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// Whether an `sp_diode` instance's `entries` give `area` as a literal zero.
pub(super) fn zero_area(l: &Lower<'_, '_, '_>, entries: &[(String, u32)]) -> bool {
    let ast = &l.cst.ast;
    entries.iter().any(|(k, v)| {
        k == "area"
            && ast.tag(Node(*v)) == Tag::Num
            && super::number::lex(l.cst.tokens.text(ast.main_token(Node(*v))))
                .is_ok_and(|n| n.long.is_none() && n.literal.mantissa == 0)
    })
}

/// Appends [`WRITE_ZERO_AREA`] for a diode at `at` whose area is a literal zero.
pub(super) fn admit(design: &ir::Design, at: ir::ItemRef, diags: &mut ir::Diags) {
    if at.kind != ir::ItemKind::Element {
        return;
    }
    let e: ir::Element = design.row(at.index);
    if e.kind != ir::ElemKind::Diode {
        return;
    }
    let area = design.semi(&e).area;
    if !area.is_none()
        && matches!(design.expr(area), ir::Expr::Num(id) if design.num(id).value() == 0.0)
    {
        diags.push_at(WRITE_ZERO_AREA, design, at, 0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Oracle: settle diode_area_zero (run115: VACASK computes area 1 where the IR's zero area
    /// is ngspice-45's no device): a diode at literal area 0 is refused, any other is not.
    #[test]
    fn a_zero_diode_area_is_refused() {
        let refused = |area: f64| {
            let mut design = ir::Design::new();
            let at = {
                let mut b = design.builder();
                let (_, top) = b.file("t.sim", ir::Role::Deck);
                let head = b.head("d1", &["a", "0"], Some("dm"), &[], ir::ExprId::NONE);
                let row = ir::semi::Semi {
                    area: b.num(area),
                    ic: ir::List::EMPTY,
                    temp_abs: ir::ExprId::NONE,
                    dtemp: ir::ExprId::NONE,
                    ic_mode: ir::passive::IcMode::None,
                    initial: ir::semi::InitialState::Unset,
                };
                b.semi(top, ir::ElemKind::Diode, head, row, ir::Span::NONE)
            };
            let mut diags = ir::Diags::new();
            admit(&design, at, &mut diags);
            diags.rows().iter().any(|d| d.code == WRITE_ZERO_AREA)
        };
        assert!(refused(0.0));
        assert!(!refused(1.0));
        assert!(!refused(2.0));
    }
}
