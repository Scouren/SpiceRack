//! Reference-evaluator bindings for the structure passes' tests: a parameter read is its
//! definition's value, evaluated the same way; nothing else is bound.
//!
//! Contents: `pub struct ByDefinition<'d>(pub &'d ir::Design)`, `ir::Bindings` for it

use spicerack::ir;

/// Bindings that read a parameter as its definition's value (a `Param` item's), evaluated the
/// same way; nothing else is bound.
pub struct ByDefinition<'d>(pub &'d ir::Design);

impl ir::Bindings for ByDefinition<'_> {
    fn param(&self, r: ir::Ref) -> Option<f64> {
        let def = r.is_resolved().then(|| self.0.def(r.def))?;
        (def.item.kind == ir::ItemKind::Param).then_some(())?;
        let value = self.0.row::<ir::item::Param>(def.item.index).value;
        ir::eval(self.0, value, self).ok()
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        None
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        None
    }
    fn time(&self) -> Option<f64> {
        None
    }
    fn temp(&self) -> Option<f64> {
        None
    }
    fn freq(&self) -> Option<f64> {
        None
    }
    fn tstep(&self) -> Option<f64> {
        None
    }
    fn tstop(&self) -> Option<f64> {
        None
    }
    fn tnom(&self) -> Option<f64> {
        None
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, _: &[f64]) -> Option<f64> {
        None
    }
}
