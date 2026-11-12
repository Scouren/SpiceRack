//! Pass `long_literal_to_shortest`: a design in, the same design out with every literal that
//! keeps more digits than its mantissa holds (`NumForm::Long`, A18) written as the shortest
//! decimal of its correctly rounded double (`Literal::from_f64`), with note `U0111` at each item
//! it changed: the IR's value is unchanged, but a simulator that converted the long digits
//! itself may not have rounded them correctly (R37). A literal is no function or operator, so
//! one row covers both forms.
//!
//! Removes `Extra(LongLiteral)`, requires `Expr(Num)`.
//!
//! Cites: `rewrite/RULINGS.md` R37; `rewrite/AMEND-A12.md` A18.
//!
//! Contents: `pub fn long_literal_to_shortest(&mut ir::Design, &mut ir::Diags)`

use super::DIGITS_ROUNDED;
use super::nodes::carrying;
use crate::ir;

/// Writes every `Long` literal as the shortest decimal of its value, with a note at each item
/// that held one.
pub fn long_literal_to_shortest(design: &mut ir::Design, diags: &mut ir::Diags) {
    let feature = ir::Feature::Extra(ir::Extra::LongLiteral);
    let held = carrying(design, feature);
    ir::rewrite_exprs(design, &mut |b, e| {
        let ir::Expr::Num(n) = b.design().expr(e) else {
            return None;
        };
        let l = b.design().num(n);
        if l.form != ir::NumForm::Long {
            return None;
        }
        let value = b.design().literal_value(l);
        Some(b.literal(ir::Literal::from_f64(value)))
    });
    for item in held {
        diags.push_at(DIGITS_ROUNDED, design, item, u32::from(feature.index()));
    }
}
