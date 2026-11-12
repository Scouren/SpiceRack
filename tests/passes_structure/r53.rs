//! R53's rule as an evaluator's bindings, written from A29 and apart from the pass: the value
//! of a subcircuit formal at one instance. The instance's actual when it passes one; else the
//! default, where an unbound parameter read is looked up from the instance's scope, a read of
//! a sibling formal is that formal's value at the same instance, and any other read is its
//! definition's value. Used before `formal_defaults_to_instances`; after it, `strict`: every
//! read an instance's value takes is bound, and an unbound one is out of the domain.
//!
//! Contents: `pub struct AtInstance`, `AtInstance::new`, `AtInstance::value(&self, usize) -> Option<f64>`,
//! `ir::Bindings` for it, `pub enum Read`, `pub fn keeps_every_instances_values(&str, fn(&mut ir::Design, &mut ir::Diags), Read)`,
//! `pub fn values(&ir::Design, bool) -> Vec<Option<u64>>`, `pub fn design(u64, Read) -> String`

use std::cell::Cell;

use spicerack::ir;

use crate::bindings::ByDefinition;

/// How a seeded default reads a top parameter (R62, A37): unbound (`u?`, looked up from each
/// instance's scope) or bound where the definition stands (`u`, the lexical rule), with the
/// wrapper `w` shadowing some of the names either way.
#[derive(Clone, Copy)]
pub enum Read {
    Unbound,
    Lexical,
}

/// One instance of one subcircuit, read the way ngspice reads its formals (R53, A29).
pub struct AtInstance<'d> {
    design: &'d ir::Design,
    formals: Vec<(ir::NameId, ir::ExprId, ir::DefId)>,
    given: Vec<(ir::NameId, ir::ExprId)>,
    scope: ir::ScopeId,
    depth: Cell<usize>,
    /// Refuse an unbound read (after the pass, every read an actual holds is bound).
    pub strict: bool,
}

impl<'d> AtInstance<'d> {
    /// The instance `inst`, listed in `scope`, of the subcircuit `subckt`.
    pub fn new(
        design: &'d ir::Design,
        subckt: ir::ItemRef,
        inst: ir::ItemRef,
        scope: ir::ScopeId,
    ) -> Self {
        let s = design.row::<ir::item::Subckt>(subckt.index);
        let formals = design
            .params(s.params)
            .iter()
            .map(|&(n, v)| (n, v, design.lookup(s.body, ir::DefKind::Param, n)))
            .collect();
        let given = design
            .params(design.row::<ir::Element>(inst.index).params)
            .to_vec();
        AtInstance {
            design,
            formals,
            given,
            scope,
            depth: Cell::new(0),
            strict: false,
        }
    }

    /// The value of formal `k` at this instance, or `None` outside the evaluator's domain.
    pub fn value(&self, k: usize) -> Option<f64> {
        let (name, default, _) = self.formals[k];
        let root = self
            .given
            .iter()
            .find(|g| g.0 == name)
            .map_or(default, |g| g.1);
        if root.is_none() || self.depth.get() > self.formals.len() {
            return None;
        }
        self.depth.set(self.depth.get() + 1);
        let v = ir::eval(self.design, root, self).ok();
        self.depth.set(self.depth.get() - 1);
        v
    }
}

impl ir::Bindings for AtInstance<'_> {
    fn param(&self, r: ir::Ref) -> Option<f64> {
        let d = self.design;
        if !r.is_resolved() {
            if self.strict {
                return None;
            }
            let def = d.lookup(self.scope, ir::DefKind::Param, r.name);
            let read = ir::Ref { name: r.name, def };
            return (!def.is_none())
                .then(|| ByDefinition(d).param(read))
                .flatten();
        }
        match self.formals.iter().position(|f| f.2 == r.def) {
            Some(k) => self.value(k),
            None => ByDefinition(d).param(r),
        }
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

/// R53's property over 200 seeded designs for the row `name` (the test's doc in
/// `tests/passes_structure.rs`), their defaults' reads `Read::Lexical` or `Read::Unbound`.
pub fn keeps_every_instances_values(
    name: &str,
    run: fn(&mut ir::Design, &mut ir::Diags),
    read: Read,
) {
    let (mut designs, mut points, mut passed, mut changed) = (0, 0, 0, 0);
    for seed in 1..=200u64 {
        let text = design(seed, read);
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(&text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "seed {seed}: {:?}\n{text}", diags.rows());
        let before = values(&design, false);
        let text_before = dump(&design);
        run(&mut design, &mut diags);
        changed += usize::from(dump(&design) != text_before);
        let mut found = ir::Diags::new();
        ir::check(&design, &mut found);
        assert!(found.is_empty(), "seed {seed}: {:?}\n{text}", found.rows());
        let after = values(&design, true);
        assert_eq!(after, before, "seed {seed}\n{text}");
        designs += 1;
        points += before.iter().flatten().count();
        passed += diags.rows().is_empty() as usize;
    }
    assert!(
        designs == 200 && points > 1000 && passed == 200 && changed > 100,
        "{name}: {designs} {points} {passed} {changed} changed"
    );
    eprintln!("{name}: {designs} seeded designs, {changed} changed, {points} instance values");
}

/// Every formal's value at every instance of the design, in listed order.
pub fn values(d: &ir::Design, strict: bool) -> Vec<Option<u64>> {
    let mut out = Vec::new();
    for (i, s) in d.reachable() {
        if i.kind != ir::ItemKind::Element {
            continue;
        }
        let m = d.row::<ir::Element>(i.index).model;
        if !m.is_resolved() || d.def(m.def).item.kind != ir::ItemKind::Subckt {
            continue;
        }
        let subckt = d.def(m.def).item;
        let n = d
            .params(d.row::<ir::item::Subckt>(subckt.index).params)
            .len();
        let mut at = AtInstance::new(d, subckt, i, s);
        at.strict = strict;
        out.extend((0..n).map(|k| at.value(k).map(f64::to_bits)));
    }
    out
}

/// One seeded design of R53's shape, as IR text: each default's reads of `u`, `v`, `g` as
/// `read` says.
pub fn design(seed: u64, read: Read) -> String {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = move |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    let params = ["u", "v", "g"];
    let mark = match read {
        Read::Unbound => "?",
        Read::Lexical => "",
    };
    let mut t = String::from("file \"a.sp\" deck\n");
    for p in params {
        t += &format!("  param {p} = {}\n", 1 + next(9));
    }
    // formals: constant, an unbound read, a sibling read (of an earlier formal), mixed
    let n = 2 + next(4) as usize;
    let mut head = Vec::new();
    for k in 0..n {
        let p = params[next(3) as usize];
        let default = match (next(4), k) {
            (0, _) => format!("{}", 1 + next(5)),
            (1, _) | (_, 0) => format!("{p}{mark}"),
            (2, _) => format!("(f{} * 2)", next(k as u64)),
            _ => format!("(f{} + {p}{mark})", next(k as u64)),
        };
        head.push(format!("f{k}={default}"));
    }
    t += &format!("  subckt s (a b) {}\n", head.join(" "));
    t += "    element resistor r1 (a b) value=f0\n";
    let instance = |name: &str, next: &mut dyn FnMut(u64) -> u64| {
        let mut given = Vec::new();
        for k in 0..n {
            if next(3) == 0 {
                given.push(format!("f{k}={}", 1 + next(7)));
            }
        }
        match given.is_empty() {
            true => format!("element instance {name} (a 0) model=s"),
            false => format!(
                "element instance {name} (a 0) model=s params=({})",
                given.join(" ")
            ),
        }
    };
    t += "  subckt w (a b)\n";
    for p in params {
        if next(2) == 0 {
            t += &format!("    param {p} = {}\n", 10 + next(90));
        }
    }
    for k in 0..1 + next(3) {
        t += &format!("    {}\n", instance(&format!("xw{k}"), &mut next));
    }
    for k in 0..1 + next(3) {
        t += &format!("  {}\n", instance(&format!("xt{k}"), &mut next));
    }
    t += "  element instance xw (n 0) model=w\n";
    t
}

// ---- private ----

/// The design's IR text.
fn dump(design: &ir::Design) -> String {
    let mut out = String::new();
    ir::dump(design, design.entry(), &mut out);
    out
}
