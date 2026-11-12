//! Instance statements → `Element` rows: `name (nodes) master params` of a model, a subcircuit
//! or a loaded module (the kind from the master, `masters::find`), `$mfactor` → `Element.mult`
//! on a device (a parameter on a subckt instance), and the behavioural source `name (p n)
//! v=|i=<expr>` → `Behav`. The master table every file declares (`declare`) and its
//! lookup live in `read_elements/masters.rs`; the spine calls `declare` and binds the masters
//! (`bind::masters`).
//!
//! Rules (each with its evidence):
//! - the master resolves in the own subckt, then the top, never an enclosing subckt (VD
//!   cir-masters; run m1: `InvalidInSource`, R2003); a master no read file defines is
//!   `Unresolved` (R2002 note), its parameters as written;
//! - an instance with fewer terminals than its master's ports (VACASK connects the rest to
//!   internal nodes, run fwd) has no IR terminal for an own internal node: `Private` (P2104, R41
//!   Q3); more terminals than ports is VACASK's error (`InvalidInSource`, P2104);
//! - a model of a proven family is its element kind (`families::kind`), its typed fields read:
//!   R/C/L `r`/`resistance`, `c`/`capacitance`, `l`/`inductance` → `value`, `temp` → `temp_abs`,
//!   `dtemp`, `tc`/`tc1` → `tc1`, `tc2`, `tce`, `scale`, `noisy`/`noise` 0 or 1 → `noisy` (runs
//!   tc, prc: ΔT = T − tnom, the direct law, `scale` multiplies R: `TcBase::Tnom`,
//!   `TcLaw::Direct`); diode `area`, `temp`, `dtemp` → `Semi`; every other instance parameter
//!   by the module's name for it (`instance_<p>` → `<p>`); a model parameter or an unknown name
//!   is VACASK's error (S 7.11, P2103); `$mfactor` → `mult` (VD cir-mfactor);
//! - a subckt instance's parameters are the subckt's: `$mfactor` only when it declares one (run
//!   n2, R2005), any other undeclared name refused (run c1, R2004);
//! - a behavioural source: `v=`/`potential=` a voltage, `i=`/`flow=` a current, in
//!   `Context::Behavioural`; one that reads a parameter waits for A21's `ToReal` (run b1, R41 Q1:
//!   `Unread`); any other form `Private`.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (instance, `$mfactor`, R C L, behavioural); VD
//! cir-instance, cir-mfactor, dev-builtin-behavioral.
//!
//! Contents: `FEATURES`, `CODES`, `UNRESOLVED`, `NOT_FORMAL`, `NO_MFACTOR`, `TERMINALS`, `NOISY`,
//! `Declared`, `declare`, `instance`, `instance_in`
//! (crate-private)

mod behavioural;
mod masters;

pub(super) use masters::Declared;

use super::expr_read::{self, Context, Refusal};
use super::families::{self, Name};
use super::read::{self, Cst, Lower};
use super::read_models::UNKNOWN;
use super::read_structure::ENCLOSING;
use super::write_models;
use crate::ir;
use crate::syntax::ast::{Node, Tag};
use masters::{Found, Master};

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Element))
    .with(ir::Feature::Elem(ir::ElemKind::Resistor))
    .with(ir::Feature::Elem(ir::ElemKind::Capacitor))
    .with(ir::Feature::Elem(ir::ElemKind::Inductor))
    .with(ir::Feature::Elem(ir::ElemKind::Diode))
    .with(ir::Feature::Elem(ir::ElemKind::Mosfet))
    .with(ir::Feature::Elem(ir::ElemKind::ModuleModel))
    .with(ir::Feature::Elem(ir::ElemKind::Instance))
    .with(ir::Feature::Elem(ir::ElemKind::Unresolved))
    .with(ir::Feature::Elem(ir::ElemKind::Behav))
    .with(ir::Feature::Circuit(ir::CircuitExtra::MultResistor))
    .with(ir::Feature::Circuit(ir::CircuitExtra::MultCapacitor))
    .with(ir::Feature::Circuit(ir::CircuitExtra::MultInductor))
    .with(ir::Feature::Circuit(ir::CircuitExtra::MultSemi))
    .with(ir::Feature::Circuit(ir::CircuitExtra::MultModule))
    .with(ir::Feature::Circuit(ir::CircuitExtra::PassiveNoiseless))
    .with(ir::Feature::Circuit(ir::CircuitExtra::PassiveScale))
    .with(ir::Feature::Circuit(ir::CircuitExtra::PassiveTc))
    .with(ir::Feature::Circuit(ir::CircuitExtra::PassiveTce))
    .with(ir::Feature::Circuit(ir::CircuitExtra::InstanceTemp))
    .with(ir::Feature::Circuit(ir::CircuitExtra::InstanceDtemp))
    .with(ir::Feature::Circuit(ir::CircuitExtra::SemiArea))
    .with(ir::Feature::Circuit(ir::CircuitExtra::NameWithoutLetter));

/// A master no read file defines.
pub(super) const UNRESOLVED: ir::Code = ir::Code::new(b'R', 2002);
/// A dependent or undeclared subckt parameter given on an instance.
pub(super) const NOT_FORMAL: ir::Code = ir::Code::new(b'R', 2004);
/// `$mfactor` on a subckt that does not declare it.
pub(super) const NO_MFACTOR: ir::Code = ir::Code::new(b'R', 2005);
/// An instance whose terminal count is not its master's.
pub(super) const TERMINALS: ir::Code = ir::Code::new(b'P', 2104);
/// A `noisy=` that is not the literal 0 or 1.
pub(super) const NOISY: ir::Code = ir::Code::new(b'P', 2106);

/// This area's rows, and the ones the model and element writers push.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        UNRESOLVED,
        ir::Info {
            title: "an instance whose master no read file defines",
            cite: "VD cir-masters; IR-CIRCUIT §2 (the unresolved kind)",
            explain: "The master is neither a model nor a subcircuit of any file read; the instance is kept as `Unresolved`, its parameters as written.",
            kind: ir::Kind::Note,
        },
    ),
    (
        NOT_FORMAL,
        ir::Info {
            title: "an instance parameter the subcircuit does not declare",
            cite: "DIALECT-VACASK runs c1, t2 (\"Parameter 'x' not found.\")",
            explain: "A subcircuit instance may give only the parameters its subcircuit declares; VACASK refuses any other name.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        NO_MFACTOR,
        ir::Info {
            title: "$mfactor on a subcircuit that does not declare it",
            cite: "DIALECT-VACASK run n2 (\"Parameter '$mfactor' not found.\"); VD cir-mfactor",
            explain: "On a subcircuit instance `$mfactor` is an ordinary parameter; VACASK refuses it unless the subcircuit's parameters declare it.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        TERMINALS,
        ir::Info {
            title: "an instance whose terminal count is not its master's",
            cite: "VD cir-instance (Unconnected terminal handling); run fwd; RULINGS R41 (Q3)",
            explain: "VACASK connects the trailing terminals an OSDI or subcircuit instance leaves out to internal nodes; the IR has no terminal for an own internal node, so the statement is kept as VACASK's text (more terminals than the master has is VACASK's error).",
            kind: ir::Kind::Note,
        },
    ),
    (
        NOISY,
        ir::Info {
            title: "a noisy= that is not 0 or 1",
            cite: "spice/resistor.va (`noisy` an integer switch)",
            explain: "`Passive::noisy` states on or off; an expression has no IR field, so the statement is kept as VACASK's text.",
            kind: ir::Kind::Note,
        },
    ),
    (
        write_models::UNSPELLED,
        ir::Info {
            title: "a name VACASK cannot spell",
            cite: "VD input-identifiers (a quoted name holds no white space)",
            explain: "VACASK reads a name as one token; a name holding white space or a line break has no spelling.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        write_models::LIBRARIES,
        ir::Info {
            title: "two device libraries of one family from files of different modules",
            cite: "RULINGS R17, R41; DIALECT-VACASK run mod (one module per shipped file)",
            explain: "A VACASK model line names one module; when the design loads a family from files of two modules, no module word states the family's model.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        write_models::LACKED,
        ir::Info {
            title: "a parameter the VACASK module does not have",
            cite: "INV-semantics 7.11 (VACASK refuses an unknown parameter); families/params.txt",
            explain: "The model or instance gives a parameter the shipped module VACASK would instantiate does not have; VACASK refuses the deck.",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// What an instance parameter of a device is.
enum Slot {
    /// `Element.mult`.
    Mult,
    /// A typed field of the detail row.
    Field(&'static str),
    /// An instance parameter by its IR name.
    Param(String),
}

/// A `Semi` with every field absent.
const SEMI: ir::semi::Semi = ir::semi::Semi {
    area: ir::ExprId::NONE,
    ic: ir::List::EMPTY,
    temp_abs: ir::ExprId::NONE,
    dtemp: ir::ExprId::NONE,
    ic_mode: ir::passive::IcMode::None,
    initial: ir::semi::InitialState::Unset,
};

/// The masters of every file (one walk before lowering; `Lower::declared`).
#[allow(dead_code, reason = "the spine calls it once Lower::declared lands")]
pub(super) fn declare(csts: &[(Cst<'_>, ir::ScopeId)]) -> Declared {
    masters::declare(csts)
}

/// `name (nodes) master params`, and a behavioural source (no master).
///
/// Until the spine carries the declared-master table (`Lower::declared`, asked of
/// vacask-plan) every instance is `Unread`; `instance_in` is the body.
pub(super) fn instance(l: &mut Lower<'_, '_, '_>, n: Node) {
    read::unread(l, n);
}

/// The body of `instance`, given every file's declared masters.
#[allow(dead_code, reason = "instance calls it once Lower::declared lands")]
pub(super) fn instance_in(l: &mut Lower<'_, '_, '_>, n: Node, declared: &Declared) {
    let h = super::bind::head(l, n);
    let (name, nodes) = (h.name, h.terminals);
    let mut entries = Vec::new();
    for &f in &h.args {
        if l.cst.ast.tag(Node(f)) != Tag::Assign {
            return read::opaque(l, n, ir::OpaqueClass::Private, Some(super::codes::PRIVATE));
        }
        let key = read::field_name(l.cst, l.cst.ast.lhs(Node(f)));
        entries.push((key, l.cst.ast.rhs(Node(f))));
    }
    let Some(master) = h.master else {
        return behavioural::source(l, n, &name, &nodes, &entries);
    };
    match masters::find(l, declared, &master) {
        Found::Enclosing => read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(ENCLOSING)),
        Found::Nowhere => unresolved(l, n, &name, &nodes, &master, &entries),
        Found::Here(Master::Subckt { ports, params }) => {
            subckt(l, n, (&name, &nodes, &master), ports, &params, &entries);
        }
        Found::Here(Master::Model(word)) => device(l, n, (&name, &nodes, &master), &word, &entries),
    }
}

// ---- private ----

/// Checks every value of `entries` in `ctx` (A2: before the first push).
fn check(l: &Lower<'_, '_, '_>, entries: &[(String, u32)], ctx: Context) -> Result<(), Refusal> {
    entries
        .iter()
        .try_for_each(|&(_, v)| expr_read::check_expr(l, v, ctx))
}

/// The element header, its master named and bound when already defined (else pending).
fn head(
    l: &mut Lower<'_, '_, '_>,
    (name, nodes, master): (&str, &[String], &str),
    params: &[(String, ir::ExprId)],
    mult: ir::ExprId,
) -> ir::ElemHead {
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let params: Vec<(&str, ir::ExprId)> = params.iter().map(|(k, e)| (k.as_str(), *e)).collect();
    l.b.head(name, &nodes, Some(master), &params, mult)
}

/// An instance of a master no read file defines.
fn unresolved(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    name: &str,
    nodes: &[String],
    master: &str,
    entries: &[(String, u32)],
) {
    if let Err(r) = check(l, entries, Context::Param) {
        return read::opaque(l, n, r.class, Some(r.code));
    }
    let params = build(l, entries);
    let head = head(l, (name, nodes, master), &params, ir::ExprId::NONE);
    let span = l.span(n);
    l.b.instance(l.scope, ir::ElemKind::Unresolved, head, span);
    l.diags.push(UNRESOLVED, span, 0);
}

/// An instance of a subcircuit.
fn subckt(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    who: (&str, &[String], &str),
    ports: usize,
    declared: &[String],
    entries: &[(String, u32)],
) {
    if who.1.len() != ports {
        let class = match who.1.len() < ports {
            true => ir::OpaqueClass::Private,
            false => ir::OpaqueClass::InvalidInSource,
        };
        return read::opaque(l, n, class, Some(TERMINALS));
    }
    for (key, _) in entries {
        if !declared.contains(key) {
            let code = match key.as_str() {
                "$mfactor" => NO_MFACTOR,
                _ => NOT_FORMAL,
            };
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(code));
        }
    }
    if let Err(r) = check(l, entries, Context::Param) {
        return read::opaque(l, n, r.class, Some(r.code));
    }
    let params = build(l, entries);
    let head = head(l, who, &params, ir::ExprId::NONE);
    let span = l.span(n);
    let item = l.b.instance(l.scope, ir::ElemKind::Instance, head, span);
    l.bind_master_later(item);
}

/// An instance of a model of the module `word`.
fn device(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    who: (&str, &[String], &str),
    word: &str,
    entries: &[(String, u32)],
) {
    let proven = families::module(word).filter(|m| families::PROVEN.contains(&m.word));
    let family = proven.map_or(ir::ModelFamily::Module, |m| m.family);
    let kind = families::kind(family);
    if let Some(m) = families::module(word)
        && who.1.len() != usize::from(m.terminals)
    {
        let class = match who.1.len() < usize::from(m.terminals) {
            true => ir::OpaqueClass::Private,
            false => ir::OpaqueClass::InvalidInSource,
        };
        return read::opaque(l, n, class, Some(TERMINALS));
    }
    let mut slots = Vec::with_capacity(entries.len());
    for (key, _) in entries {
        let slot = match (key.as_str(), families::instance_name_in(word, key)) {
            ("$mfactor", _) => Slot::Mult,
            (_, Name::Ir(ir)) => match field(kind, &families::canonical(word, key)) {
                Some(f) if family != ir::ModelFamily::Module => Slot::Field(f),
                _ if family == ir::ModelFamily::Module => Slot::Param(key.clone()),
                _ => Slot::Param(ir),
            },
            _ => return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(UNKNOWN)),
        };
        slots.push(slot);
    }
    if let Err(r) = check(l, entries, Context::Param) {
        return read::opaque(l, n, r.class, Some(r.code));
    }
    let noisy = entries
        .iter()
        .zip(&slots)
        .find(|(_, s)| matches!(s, Slot::Field("noisy")))
        .map(|((_, v), _)| zero_one(l, *v));
    if noisy == Some(None) {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(NOISY));
    }
    let mut mult = ir::ExprId::NONE;
    let mut params = Vec::new();
    let mut passive = ir::passive::Passive::PLAIN;
    let mut semi = SEMI;
    for ((key, value), slot) in entries.iter().zip(slots) {
        if matches!(slot, Slot::Field("noisy")) {
            continue;
        }
        let e = expr_read::expr(l, *value, Context::Param).expect("checked");
        match slot {
            Slot::Mult => mult = e,
            Slot::Param(ir) => params.push((ir, e)),
            Slot::Field(f) => set(&mut passive, &mut semi, f, e),
        }
        let _ = key;
    }
    passive.noisy = noisy.flatten().unwrap_or(true);
    let head = head(l, who, &params, mult);
    let span = l.span(n);
    let item = match kind.detail() {
        ir::DetailKind::Passive => l.b.passive(l.scope, kind, head, passive, span),
        ir::DetailKind::Semi => l.b.semi(l.scope, kind, head, semi, span),
        _ => l.b.instance(l.scope, kind, head, span),
    };
    l.bind_master_later(item);
}

/// The typed field a module's (canonical) instance parameter `p` is on an element of `kind`.
fn field(kind: ir::ElemKind, p: &str) -> Option<&'static str> {
    use ir::ElemKind as K;
    Some(match (kind, p) {
        (K::Resistor, "resistance" | "r")
        | (K::Capacitor, "capacitance" | "c")
        | (K::Inductor, "inductance" | "l") => "value",
        (K::Resistor | K::Capacitor | K::Inductor, "tc" | "tc1") => "tc1",
        (K::Resistor | K::Capacitor | K::Inductor, "tc2") => "tc2",
        (K::Resistor, "tce") => "tce",
        (K::Resistor | K::Capacitor | K::Inductor, "scale") => "scale",
        (K::Resistor, "noisy") => "noisy",
        (K::Resistor | K::Capacitor | K::Inductor | K::Diode | K::Mosfet, "temp") => "temp",
        (K::Resistor | K::Capacitor | K::Inductor | K::Diode | K::Mosfet, "dtemp") => "dtemp",
        (K::Diode, "area") => "area",
        _ => return None,
    })
}

/// Sets field `f` of the passive or semiconductor row.
fn set(passive: &mut ir::passive::Passive, semi: &mut ir::semi::Semi, f: &str, e: ir::ExprId) {
    match f {
        "value" => passive.value = e,
        "tc1" => passive.tc1 = e,
        "tc2" => passive.tc2 = e,
        "tce" => passive.tce = e,
        "scale" => passive.scale = e,
        "area" => semi.area = e,
        "temp" => (passive.temp_abs, semi.temp_abs) = (e, e),
        _ => (passive.dtemp, semi.dtemp) = (e, e),
    }
}

/// `Some(false)` for the literal 0, `Some(true)` for 1, `None` for anything else.
fn zero_one(l: &Lower<'_, '_, '_>, value: u32) -> Option<bool> {
    let ast = &l.cst.ast;
    if ast.tag(Node(value)) != Tag::Num {
        return None;
    }
    let lit = super::number::lex(l.cst.tokens.text(ast.main_token(Node(value))))
        .ok()?
        .literal;
    let int = |v| ir::Literal {
        mantissa: v,
        frac: 0,
        exp10: 0,
        form: ir::NumForm::Int,
    };
    match (lit.eq_value(int(0)), lit.eq_value(int(1))) {
        (true, _) => Some(false),
        (_, true) => Some(true),
        _ => None,
    }
}

/// Each entry's value built, under its name as written.
fn build(l: &mut Lower<'_, '_, '_>, entries: &[(String, u32)]) -> Vec<(String, ir::ExprId)> {
    entries
        .iter()
        .map(|(k, v)| {
            let e = expr_read::expr(l, *v, Context::Param).expect("checked");
            (k.clone(), e)
        })
        .collect()
}
