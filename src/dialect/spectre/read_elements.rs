//! Instance statements → `Element` rows, and `model` statements → `Model` rows. An instance
//! is `name (nodes) master k=v …` (MAN; every RUN-SP deck); its master decides what it is: a
//! primitive (`resistor`, `capacitor`, `inductor`, the sources and controlled sources, `bsource`),
//! a model (its own master decides the kind), a subcircuit, or nothing any file declares
//! (`Unresolved`, its parameters as written). An instance name has no letter prefix.
//!
//! Rules (`rewrite/DIALECT-SPECTRE.md` §5, §6, each with its evidence):
//! - `resistor r=`, `capacitor c=`, `inductor l=` the value (RUN-SP `p_elements`); `r=` and `c=`
//!   may read the circuit (RUN-SP `t_m4_dependent`); `m=` the multiplicity, `trise=` the
//!   temperature offset, `tc1=`/`tc2=` the coefficients, `isnoisy=no` no thermal noise (MAN; the
//!   `t_m1`/`t_m2` decks ran: RUN-SP);
//! - `vcvs gain=`, `vccs gm=`, `cccs probe= gain=`, `ccvs probe= rm=` (RUN-SP `p_elements`);
//! - `bsource v=` a voltage, `i=` a current, `q=` a charge (RUN-SP `p_behavioural`,
//!   `p_charge_capacitor`); its other outputs are not read yet;
//! - `model name diode|bjt|mos1|mos2|mos3|resistor|capacitor|inductor …` with `type=` the
//!   polarity (MAN SR19 p.498; RUN-SP `p_models` for diode, bjt, mos1); `level` and `version`
//!   cards, model groups and other masters are not read yet;
//! - a model instance's `area=` (diode, BJT), `m=`, `trise=`; every other parameter by its name;
//! - an element name given twice in a scope, and a subcircuit instance inside its own subcircuit:
//!   `InvalidInSource` (MAN);
//! - a parameter name with an upper-case letter on a primitive or a model: Spectre ignores it
//!   with SFE-102 and runs the default (RUN-SP: `W=`, `L=` on a MOS gave the default size), a
//!   silent wrong number: `InvalidInSource`; a parameter given twice: `InvalidInSource`.
//!
//! Contents: `FEATURES`, `Declared`, `Master`, `declare`, `instance`, `model`, `Fields`, `fields`,
//! `value` (crate-private)

use std::collections::HashMap;

use super::codes;
use super::expr_read::{self, Context};
use super::read::{self, Cst, Lower};
use super::read_sources;
use crate::ir;
use crate::ir::CircuitExtra as X;
use crate::ir::Feature as F;
use crate::syntax::ast::{NONE, Node, Tag};

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(F::Item(ir::ItemKind::Element))
    .with(F::Item(ir::ItemKind::Model))
    .with(F::Elem(ir::ElemKind::Resistor))
    .with(F::Elem(ir::ElemKind::Capacitor))
    .with(F::Elem(ir::ElemKind::Inductor))
    .with(F::Elem(ir::ElemKind::Vcvs))
    .with(F::Elem(ir::ElemKind::Vccs))
    .with(F::Elem(ir::ElemKind::Cccs))
    .with(F::Elem(ir::ElemKind::Ccvs))
    .with(F::Elem(ir::ElemKind::Behav))
    .with(F::Elem(ir::ElemKind::ChargeCapacitor))
    .with(F::Elem(ir::ElemKind::Diode))
    .with(F::Elem(ir::ElemKind::Bjt))
    .with(F::Elem(ir::ElemKind::Mosfet))
    .with(F::Elem(ir::ElemKind::Instance))
    .with(F::Elem(ir::ElemKind::Unresolved))
    .with(F::Family(ir::ModelFamily::Resistor))
    .with(F::Family(ir::ModelFamily::Capacitor))
    .with(F::Family(ir::ModelFamily::Inductor))
    .with(F::Family(ir::ModelFamily::DiodeSpectre))
    .with(F::Family(ir::ModelFamily::GummelPoon))
    .with(F::Family(ir::ModelFamily::Mos1))
    .with(F::Family(ir::ModelFamily::Mos2))
    .with(F::Family(ir::ModelFamily::Mos3))
    .with(F::Circuit(X::NameWithoutLetter))
    .with(F::Circuit(X::ElementWithoutModel))
    .with(F::Circuit(X::ModelWithoutLibrary))
    .with(F::Circuit(X::PassiveDynamicValue))
    .with(F::Circuit(X::PassiveNoiseless))
    .with(F::Circuit(X::PassiveTc))
    .with(F::Circuit(X::InstanceDtemp))
    .with(F::Circuit(X::SemiArea))
    .with(F::Circuit(X::MultResistor))
    .with(F::Circuit(X::MultCapacitor))
    .with(F::Circuit(X::MultInductor))
    .with(F::Circuit(X::MultSemi))
    .with(F::Circuit(X::MultInstanceParallel));

/// What a name some `model` or `subckt` statement declares is (the first declaration wins).
pub(super) type Declared = HashMap<String, Master>;

/// A declared master.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Master {
    /// A model, with its master word (`diode`, `bjt`, …).
    Model(String),
    /// A subcircuit.
    Subckt,
}

/// An instance or model statement's `key=value` fields: (key, value node), in order.
pub(super) type Fields = Vec<(String, u32)>;

/// What a refused element is kept as.
type Fail = (ir::OpaqueClass, ir::Code);

/// Every model and subcircuit name of `csts`, in any scope.
pub(super) fn declare<'c, 's: 'c>(csts: impl Iterator<Item = &'c Cst<'s>>) -> Declared {
    let mut out = Declared::new();
    for cst in csts {
        let ast = &cst.ast;
        let mut todo: Vec<u32> = ast.fields(ast.root()).to_vec();
        // Bounded: each node is taken once (the statement tree is acyclic).
        while let Some(s) = todo.pop() {
            let n = Node(s);
            let tag = ast.tag(n);
            let f = match matches!(tag, Tag::Model | Tag::Subckt) {
                true => ast.fields(n),
                false => &[],
            };
            match tag {
                Tag::Model if f.len() >= 2 => {
                    let master = Master::Model(read::field_name(cst, f[1]));
                    out.entry(read::field_name(cst, f[0])).or_insert(master);
                }
                Tag::Subckt | Tag::If | Tag::Else => {
                    if ast.tag(n) == Tag::Subckt && !f.is_empty() {
                        out.entry(read::field_name(cst, f[0]))
                            .or_insert(Master::Subckt);
                    }
                    todo.extend_from_slice(ast.body(n));
                    todo.extend(ast.next(n).map(|x| x.0));
                }
                _ => {}
            }
        }
    }
    out
}

/// `name (nodes) master k=v …`: one `Element` by the master, or an `Opaque`.
pub(super) fn instance(l: &mut Lower<'_, '_, '_>, n: Node) {
    let h = l.cst.ast.header(n);
    if h.name == NONE || h.master == NONE || !l.cst.ast.body(n).is_empty() {
        return read::unread(l, n);
    }
    let master = l.field(h.master);
    let name = l.field(h.name);
    // one element of a name in a scope (MAN: an instance name is unique in its subcircuit)
    if !l.elements.entry(l.scope).or_default().insert(name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
    }
    // a subcircuit instantiated inside itself (MAN: no recursive hierarchy)
    if recursive(l, &master) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::RECURSION);
    }
    let Some(fields) = fields(l, n) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    let primitive = matches!(
        master.as_str(),
        "resistor"
            | "capacitor"
            | "inductor"
            | "vsource"
            | "isource"
            | "vcvs"
            | "vccs"
            | "cccs"
            | "ccvs"
            | "bsource"
    );
    if primitive
        && fields
            .iter()
            .any(|(k, _)| k.bytes().any(|b| b.is_ascii_uppercase()))
    {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::UPPER_CASE);
    }
    let declared = l.declared.get(&master).cloned();
    let result = match (master.as_str(), declared) {
        ("resistor", _) => passive(l, n, ir::ElemKind::Resistor, None, &fields),
        ("capacitor", _) => passive(l, n, ir::ElemKind::Capacitor, None, &fields),
        ("inductor", _) => passive(l, n, ir::ElemKind::Inductor, None, &fields),
        ("vsource" | "isource", _) => read_sources::source(l, n, &master, &fields),
        ("vcvs" | "vccs" | "cccs" | "ccvs", _) => controlled(l, n, &master, &fields),
        ("bsource", _) => behav(l, n, &fields),
        (_, Some(Master::Model(word))) => match model_kind(&word) {
            Some((kind, _)) if kind.detail() == ir::DetailKind::Passive => {
                passive(l, n, kind, Some(&master), &fields)
            }
            Some((kind, _)) => semi(l, n, kind, &master, &fields),
            None => headed(l, n, ir::ElemKind::Unresolved, &master, &fields),
        },
        (_, Some(Master::Subckt)) => headed(l, n, ir::ElemKind::Instance, &master, &fields),
        (_, None) => headed(l, n, ir::ElemKind::Unresolved, &master, &fields),
    };
    if let Err((class, code)) = result {
        read::opaque(l, n, class, code);
    }
}

/// `model name master k=v …`: one `Model` of the master's family, or an `Opaque`.
pub(super) fn model(l: &mut Lower<'_, '_, '_>, n: Node) {
    let ast = &l.cst.ast;
    let all = ast.fields(n).to_vec();
    if all.len() < 2 || !ast.body(n).is_empty() || ast.tag(Node(all[0])) != Tag::Word {
        return read::unread(l, n);
    }
    let (name, word) = (l.field(all[0]), l.field(all[1]));
    if super::read_structure::defined_here(l, ir::DefKind::Model, &name) {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::REDEFINED);
    }
    let Some((_, family)) = model_kind(&word) else {
        return read::unread(l, n);
    };
    let Some(mut fields) = assigns(l, &all[2..]) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::MALFORMED);
    };
    if fields
        .iter()
        .any(|(k, _)| k.bytes().any(|b| b.is_ascii_uppercase()))
    {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, codes::UPPER_CASE);
    }
    if fields
        .iter()
        .any(|(k, _)| matches!(k.as_str(), "level" | "version"))
    {
        return read::unread(l, n);
    }
    let polarity = match (take(&mut fields, "type"), family) {
        (None, ir::ModelFamily::GummelPoon | ir::ModelFamily::Mos1) => ir::Polarity::N,
        (None, ir::ModelFamily::Mos2 | ir::ModelFamily::Mos3) => ir::Polarity::N,
        (None, _) => ir::Polarity::None,
        (Some(t), f) => match (l.field(t).as_str(), f) {
            ("npn", ir::ModelFamily::GummelPoon) => ir::Polarity::N,
            ("pnp", ir::ModelFamily::GummelPoon) => ir::Polarity::P,
            ("n", ir::ModelFamily::Mos1 | ir::ModelFamily::Mos2 | ir::ModelFamily::Mos3) => {
                ir::Polarity::N
            }
            ("p", ir::ModelFamily::Mos1 | ir::ModelFamily::Mos2 | ir::ModelFamily::Mos3) => {
                ir::Polarity::P
            }
            _ => return read::unread(l, n),
        },
    };
    if let Err(r) = check(l, &fields, &[]) {
        return read::opaque(l, n, r.0, r.1);
    }
    // the name first, then the parameters: the IR text's order (`ir::dump`), so a read and a
    // parse of its dump intern alike
    let name = l.b.name(&name);
    let values: Vec<(String, ir::ExprId)> = fields
        .iter()
        .map(|(k, v)| (k.clone(), value(l, *v)))
        .collect();
    let values: Vec<(&str, ir::ExprId)> = values.iter().map(|(k, e)| (k.as_str(), *e)).collect();
    let params = match values.is_empty() {
        true => ir::List::EMPTY,
        false => l.b.params(&values),
    };
    let row = ir::Model {
        name,
        params,
        bin_of: ir::NameId::NONE,
        ako: ir::Ref::unresolved(ir::NameId::NONE),
        module: ir::Ref::unresolved(ir::NameId::NONE),
        version: ir::ExprId::NONE,
        word: ir::NameId::NONE,
        family,
        polarity,
    };
    let span = l.span(n);
    l.b.model(l.scope, row, span);
}

/// Statement `n`'s fields after the master, as `key=value` pairs; `None` when one is not an
/// assignment of a plain key or a key is given twice.
pub(super) fn fields(l: &Lower<'_, '_, '_>, n: Node) -> Option<Fields> {
    assigns(l, l.cst.ast.fields(n))
}

/// The value at node `v`, read as a parameter (checked before).
pub(super) fn value(l: &mut Lower<'_, '_, '_>, v: u32) -> ir::ExprId {
    expr_read::expr(l, v, Context::Param).expect("checked")
}

// ---- private ----

/// Whether `master`, looked up from the current scope, is a subcircuit whose body encloses it.
fn recursive(l: &Lower<'_, '_, '_>, master: &str) -> bool {
    let d = l.b.design();
    let name = d.find_name(master);
    if name.is_none() {
        return false;
    }
    let def = d.lookup(l.scope, ir::DefKind::Subckt, name);
    if def.is_none() {
        return false;
    }
    let item = d.def(def).item;
    let mut at = l.scope;
    // Bounded: each step moves to the enclosing scope; the tree's depth is the parse's.
    for _ in 0..=crate::syntax::DEPTH_MAX {
        if at.is_none() {
            break;
        }
        let s = d.scope(at);
        if s.owner == item {
            return true;
        }
        at = s.parent;
    }
    false
}

/// The element kind and model family of a `model` master word.
fn model_kind(word: &str) -> Option<(ir::ElemKind, ir::ModelFamily)> {
    use ir::{ElemKind as K, ModelFamily as M};
    Some(match word {
        "resistor" => (K::Resistor, M::Resistor),
        "capacitor" => (K::Capacitor, M::Capacitor),
        "inductor" => (K::Inductor, M::Inductor),
        "diode" => (K::Diode, M::DiodeSpectre),
        "bjt" => (K::Bjt, M::GummelPoon),
        "mos1" => (K::Mosfet, M::Mos1),
        "mos2" => (K::Mosfet, M::Mos2),
        "mos3" => (K::Mosfet, M::Mos3),
        _ => return None,
    })
}

/// `key=value` pairs of `nodes`; `None` when one is not one, or a key repeats.
fn assigns(l: &Lower<'_, '_, '_>, nodes: &[u32]) -> Option<Fields> {
    let ast = &l.cst.ast;
    let mut out: Fields = Vec::with_capacity(nodes.len());
    for &f in nodes {
        let f = Node(f);
        if ast.tag(f) != Tag::Assign || ast.tag(Node(ast.lhs(f))) != Tag::Word {
            return None;
        }
        let key = l.field(ast.lhs(f));
        if out.iter().any(|(k, _)| *k == key) {
            return None;
        }
        out.push((key, ast.rhs(f)));
    }
    Some(out)
}

/// Removes and returns the value of `key`.
fn take(fields: &mut Fields, key: &str) -> Option<u32> {
    let at = fields.iter().position(|(k, _)| k == key)?;
    Some(fields.remove(at).1)
}

/// Checks every value of `fields` (those named in `circuit` may read the circuit).
fn check(l: &Lower<'_, '_, '_>, fields: &Fields, circuit: &[&str]) -> Result<(), Fail> {
    for (k, v) in fields {
        let ctx = match circuit.contains(&k.as_str()) {
            true => Context::Circuit,
            false => Context::Param,
        };
        expr_read::check_expr(l, *v, ctx).map_err(|r| (r.class, r.code))?;
    }
    Ok(())
}

/// The element head of instance `n`: its name, nodes, master (unresolved) and `params`.
fn head(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    master: Option<&str>,
    params: &[(String, ir::ExprId)],
    mult: ir::ExprId,
) -> ir::ElemHead {
    let h = l.cst.ast.header(n);
    let name = l.field(h.name);
    let nodes: Vec<String> = h.nodes.iter().map(|&x| l.field(x)).collect();
    let nodes: Vec<&str> = nodes.iter().map(String::as_str).collect();
    let params: Vec<(&str, ir::ExprId)> = params.iter().map(|(k, e)| (k.as_str(), *e)).collect();
    l.b.head(&name, &nodes, master, &params, mult)
}

/// Whether instance `n` has `count` terminals.
fn terminals(l: &Lower<'_, '_, '_>, n: Node, kind: ir::ElemKind) -> Result<(), Fail> {
    let count = l.cst.ast.header(n).nodes.len();
    let (lo, hi) = kind.terminals();
    match count >= lo as usize && count <= hi as usize {
        true => Ok(()),
        false => Err((ir::OpaqueClass::InvalidInSource, codes::TERMINALS)),
    }
}

/// A resistor, capacitor or inductor, of a model when `master` is given.
fn passive(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    kind: ir::ElemKind,
    master: Option<&str>,
    fields: &Fields,
) -> Result<(), Fail> {
    terminals(l, n, kind)?;
    let key = match kind {
        ir::ElemKind::Resistor => "r",
        ir::ElemKind::Capacitor => "c",
        _ => "l",
    };
    let circuit: &[&str] = match kind {
        ir::ElemKind::Inductor => &[],
        _ => &[key],
    };
    check(l, fields, circuit)?;
    let mut row = ir::passive::Passive::PLAIN;
    let mut mult = ir::ExprId::NONE;
    let mut params = Vec::new();
    for (k, v) in fields {
        match k.as_str() {
            k if k == key => row.value = expr_read::expr(l, *v, Context::Circuit).expect("checked"),
            "m" => mult = value(l, *v),
            "trise" => row.dtemp = value(l, *v),
            "tc1" => row.tc1 = value(l, *v),
            "tc2" => row.tc2 = value(l, *v),
            "isnoisy" => match l.field(*v).as_str() {
                "no" => row.noisy = false,
                "yes" => row.noisy = true,
                _ => return Err((ir::OpaqueClass::Unread, codes::UNREAD)),
            },
            _ => params.push((k.clone(), value(l, *v))),
        }
    }
    if row.value.is_none() && master.is_none() {
        return Err((ir::OpaqueClass::Unread, codes::NO_VALUE));
    }
    let head = head(l, n, master, &params, mult);
    let span = l.span(n);
    let item = l.b.passive(l.scope, kind, head, row, span);
    if let Some(m) = master {
        l.refer(item, 0, ir::DefKind::Model, m);
    }
    Ok(())
}

/// A diode, BJT or MOSFET of model `master`.
fn semi(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    kind: ir::ElemKind,
    master: &str,
    fields: &Fields,
) -> Result<(), Fail> {
    terminals(l, n, kind)?;
    check(l, fields, &[])?;
    let mut row = ir::semi::Semi {
        area: ir::ExprId::NONE,
        ic: ir::List::EMPTY,
        temp_abs: ir::ExprId::NONE,
        dtemp: ir::ExprId::NONE,
        ic_mode: ir::passive::IcMode::None,
        initial: ir::semi::InitialState::Unset,
    };
    let mut mult = ir::ExprId::NONE;
    let mut params = Vec::new();
    for (k, v) in fields {
        match k.as_str() {
            "area" if kind != ir::ElemKind::Mosfet => row.area = value(l, *v),
            "m" => mult = value(l, *v),
            "trise" => row.dtemp = value(l, *v),
            _ => params.push((k.clone(), value(l, *v))),
        }
    }
    let head = head(l, n, Some(master), &params, mult);
    let span = l.span(n);
    let item = l.b.semi(l.scope, kind, head, row, span);
    l.refer(item, 0, ir::DefKind::Model, master);
    Ok(())
}

/// A subcircuit instance (`m=` its multiplicity) or an instance of a master nothing declares.
fn headed(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    kind: ir::ElemKind,
    master: &str,
    fields: &Fields,
) -> Result<(), Fail> {
    check(l, fields, &[])?;
    let mut mult = ir::ExprId::NONE;
    let mut params = Vec::new();
    for (k, v) in fields {
        match (k.as_str(), kind) {
            ("m", ir::ElemKind::Instance) => mult = value(l, *v),
            _ => params.push((k.clone(), value(l, *v))),
        }
    }
    let head = head(l, n, Some(master), &params, mult);
    let span = l.span(n);
    let item = l.b.instance(l.scope, kind, head, span);
    if kind == ir::ElemKind::Instance {
        l.refer(item, 0, ir::DefKind::Subckt, master);
    }
    Ok(())
}

/// `vcvs gain=`, `vccs gm=`, `cccs probe= gain=`, `ccvs probe= rm=`.
fn controlled(
    l: &mut Lower<'_, '_, '_>,
    n: Node,
    master: &str,
    fields: &Fields,
) -> Result<(), Fail> {
    let (kind, gain_key, sensed) = match master {
        "vcvs" => (ir::ElemKind::Vcvs, "gain", false),
        "vccs" => (ir::ElemKind::Vccs, "gm", false),
        "cccs" => (ir::ElemKind::Cccs, "gain", true),
        _ => (ir::ElemKind::Ccvs, "rm", true),
    };
    terminals(l, n, kind)?;
    let mut fields = fields.clone();
    let gain = take(&mut fields, gain_key);
    let probe = take(&mut fields, "probe");
    let (Some(gain), true, true) = (gain, probe.is_some() == sensed, fields.is_empty()) else {
        return Err((ir::OpaqueClass::Unread, codes::UNREAD));
    };
    check(l, &vec![(gain_key.to_string(), gain)], &[])?;
    let sense = match probe {
        Some(p) if l.cst.ast.tag(Node(p)) == Tag::Ident => {
            let name = l.field(p);
            l.b.name(&name)
        }
        Some(_) => return Err((ir::OpaqueClass::InvalidInSource, codes::MALFORMED)),
        None => ir::NameId::NONE,
    };
    let gain = value(l, gain);
    let head = head(l, n, None, &[], ir::ExprId::NONE);
    let span = l.span(n);
    l.b.controlled(
        l.scope,
        kind,
        head,
        ir::controlled::Controlled { gain, sense },
        span,
    );
    Ok(())
}

/// `bsource v=e`, `i=e` or `q=e`: a `Behav` voltage or current, or a `ChargeCapacitor`.
fn behav(l: &mut Lower<'_, '_, '_>, n: Node, fields: &Fields) -> Result<(), Fail> {
    let [(key, v)] = fields.as_slice() else {
        return Err((ir::OpaqueClass::Unread, codes::UNREAD));
    };
    let (kind, output) = match key.as_str() {
        "v" => (ir::ElemKind::Behav, ir::controlled::OutKind::Voltage),
        "i" => (ir::ElemKind::Behav, ir::controlled::OutKind::Current),
        "q" => (
            ir::ElemKind::ChargeCapacitor,
            ir::controlled::OutKind::Charge,
        ),
        _ => return Err((ir::OpaqueClass::Unread, codes::UNREAD)),
    };
    terminals(l, n, kind)?;
    let expr = expr_read::expr(l, *v, Context::Circuit).map_err(|r| (r.class, r.code))?;
    let row = ir::controlled::Behav {
        expr,
        tc1: ir::ExprId::NONE,
        tc2: ir::ExprId::NONE,
        temp_abs: ir::ExprId::NONE,
        dtemp: ir::ExprId::NONE,
        rpar: ir::ExprId::NONE,
        output,
        tc_base: ir::passive::TcBase::Tnom,
        tc_law: ir::passive::TcLaw::Direct,
    };
    let head = head(l, n, None, &[], ir::ExprId::NONE);
    let span = l.span(n);
    l.b.behav(l.scope, kind, head, row, span);
    Ok(())
}
