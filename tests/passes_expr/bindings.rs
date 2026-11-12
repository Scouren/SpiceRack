//! The bindings the expression passes' proofs evaluate under: `Point` (every read a value
//! drawn by trial and name, or a parameter read its definition's value), `Pair` (two operands
//! bound to chosen values), and the boundary set.
//!
//! Contents: `pub struct Point`, `pub struct Pair`, `pub fn boundary() -> Vec<f64>`

use spicerack::ir;

/// Every read answers a value drawn by (trial, name) from the boundary set or a random finite
/// number; a whole value within 2⁵³ is Int on odd draws.
pub struct Point<'d> {
    pub design: &'d ir::Design,
    pub trial: u64,
    /// A resolved parameter read is its definition's value, evaluated (a head default for a
    /// formal), as the tag table assumes: the typed passes decide by it.
    pub follow: bool,
    pub depth: std::cell::Cell<u32>,
}

/// `x` and `y` bound to two values, the temperature 27, everything else 1.
pub struct Pair<'d> {
    pub design: &'d ir::Design,
    pub x: ir::Value,
    pub y: ir::Value,
}

impl ir::Bindings for Point<'_> {
    fn param(&self, p: ir::Ref) -> Option<f64> {
        self.param_value(p).map(|v| v.number)
    }
    fn param_value(&self, p: ir::Ref) -> Option<ir::Value> {
        if !self.follow || !p.is_resolved() {
            return Some(self.value(self.design.str(p.name)));
        }
        let def = self.design.def(p.def);
        let value = match (def.kind, def.item.kind) {
            (ir::DefKind::Param, ir::ItemKind::Param) => {
                self.design.row::<ir::item::Param>(def.item.index).value
            }
            (ir::DefKind::Param, ir::ItemKind::Subckt) => {
                let s: ir::item::Subckt = self.design.row(def.item.index);
                let given = self
                    .design
                    .params(s.params)
                    .iter()
                    .find(|q| q.0 == def.name);
                given.map_or(ir::ExprId::NONE, |q| q.1)
            }
            _ => return Some(ir::Value::real(self.value(self.design.str(p.name)).number)),
        };
        if value.is_none() || self.depth.get() > 64 {
            return None;
        }
        self.depth.set(self.depth.get() + 1);
        let v = ir::eval_typed(self.design, value, self).ok();
        self.depth.set(self.depth.get() - 1);
        v
    }
    fn voltage(&self, net: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(self.value(self.design.str(net)).number)
    }
    fn current(&self, element: ir::NameId) -> Option<f64> {
        Some(self.value(self.design.str(element)).number)
    }
    fn time(&self) -> Option<f64> {
        Some(self.value("$time").number)
    }
    fn temp(&self) -> Option<f64> {
        Some(self.value("$temp").number)
    }
    fn freq(&self) -> Option<f64> {
        Some(self.value("$freq").number)
    }
    fn tstep(&self) -> Option<f64> {
        Some(self.value("$tstep").number)
    }
    fn tstop(&self) -> Option<f64> {
        Some(self.value("$tstop").number)
    }
    fn tnom(&self) -> Option<f64> {
        Some(self.value("$tnom").number)
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, args: &[f64]) -> Option<f64> {
        Some(args[0])
    }
}

impl ir::Bindings for Pair<'_> {
    fn param(&self, p: ir::Ref) -> Option<f64> {
        self.param_value(p).map(|v| v.number)
    }
    fn param_value(&self, p: ir::Ref) -> Option<ir::Value> {
        Some(match self.design.str(p.name) {
            "x" => self.x,
            "y" => self.y,
            _ => ir::Value::real(1.0),
        })
    }
    fn voltage(&self, _: ir::NameId, _: ir::NameId) -> Option<f64> {
        Some(1.0)
    }
    fn current(&self, _: ir::NameId) -> Option<f64> {
        Some(1.0)
    }
    fn time(&self) -> Option<f64> {
        Some(1.0)
    }
    fn temp(&self) -> Option<f64> {
        Some(27.0)
    }
    fn freq(&self) -> Option<f64> {
        Some(1.0)
    }
    fn tstep(&self) -> Option<f64> {
        Some(1.0)
    }
    fn tstop(&self) -> Option<f64> {
        Some(1.0)
    }
    fn tnom(&self) -> Option<f64> {
        Some(27.0)
    }
    fn draw(&self, _: ir::Dist, _: ir::DrawWhen, _: ir::DrawPer, args: &[f64]) -> Option<f64> {
        Some(args[0])
    }
}

/// The boundary set: zeros, halves and their neighbours, 2⁵² ± 1, 2⁵³, the ends of the doubles.
pub fn boundary() -> Vec<f64> {
    let p52 = 4_503_599_627_370_496.0f64;
    let mut out = vec![
        0.0,
        -0.0,
        1.0,
        -1.0,
        2.0,
        3.0,
        -3.0,
        0.5,
        -0.5,
        1.5,
        2.5,
        -2.5,
        2.5f64.next_up(),
        2.5f64.next_down(),
        0.5f64.next_down(),
        -(0.5f64.next_down()),
        1.0 / 3.0,
        p52 - 1.0,
        p52,
        p52 + 1.0,
        -(p52 + 1.0),
        2.0 * p52,
        1e308,
        f64::MAX,
        f64::MIN_POSITIVE,
        5e-324,
        -1e-300,
    ];
    out.extend([1.0f64, -1.0].map(|s| s * (1.0f64).next_up()));
    out
}

// ---- private ----

impl Point<'_> {
    fn value(&self, name: &str) -> ir::Value {
        let mut h = self.trial.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xD1B5_4A32_D192_ED03;
        for b in name.bytes() {
            h = (h ^ u64::from(b)).wrapping_mul(0x100_0000_01B3);
        }
        h ^= h >> 29;
        let set = boundary();
        let number = match h % 3 {
            0 => (h >> 8) as f64 / (1u64 << 52) as f64 * 8.0 - 4.0,
            _ => set[(h >> 8) as usize % set.len()],
        };
        let whole = number.fract() == 0.0 && number.abs() <= 9_007_199_254_740_992.0;
        match whole && (h >> 40) & 1 == 1 {
            true => ir::Value::int(number),
            false => ir::Value::real(number),
        }
    }
}
