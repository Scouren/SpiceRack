//! `model name module params` → a `Model` row. The module decides the family (`families.rs`):
//! a builtin (`vsource isource vcvs vccs ccvs cccs mutual`) is a parameterless model of its
//! builtin family (A12 C5); a shipped module is a model of its family only once a run proves it
//! equals that family's reference device (R41 a, `families::PROVEN`); every other module (a
//! loaded user module, or a shipped one not yet proven) is `ModelFamily::Module`, the module
//! named and bound to a visible `DefKind::Module` when one is (else unbound, R41 Q6).
//!
//! Rules (each with its run):
//! - a builtin takes no model parameter: `model vs vsource dc=1` is "Parameter 'dc' not found."
//!   (run bp) → `InvalidInSource` (`P2103`);
//! - a parameter the shipped module lacks is a hard error (S 7.11) → `InvalidInSource`
//!   (`P2103`); an instance parameter on a model line (VD cir-instance: a default for the
//!   instances) has no IR meaning → `Private` (`P2102`, R41 Q4);
//! - a shipped module's model is a row only when every `load` of its family's files names
//!   that module's file (the writer spells the module from the design's library: R17, R41);
//!   else `Private` (`P2101`);
//! - `type=1`/`type=-1` (an integer or real literal, run t1) is the polarity of a family that
//!   has one; absent it is 1 (every shipped module's default); any other value `Private`
//!   (`P2105`);
//! - `model_<p>` is the family's `<p>` (`families::model_name_in`); `version=` is the model's
//!   version.
//!
//! Cites: `rewrite/DIALECT-VACASK.md` §3.2 (model); VD cir-masters, dev-builtin, dev-spice.
//!
//! Contents: `FEATURES`, `CODES`, `LIBRARY`, `MISPLACED`, `UNKNOWN`, `POLARITY`, `model` (crate-private)

use super::expr_read::{self, Context};
use super::families::{self, Name};
use super::read::{self, Lower};
use super::read_structure;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Model))
    .with(ir::Feature::Family(ir::ModelFamily::SourceV))
    .with(ir::Feature::Family(ir::ModelFamily::SourceI))
    .with(ir::Feature::Family(ir::ModelFamily::Vcvs))
    .with(ir::Feature::Family(ir::ModelFamily::Vccs))
    .with(ir::Feature::Family(ir::ModelFamily::Ccvs))
    .with(ir::Feature::Family(ir::ModelFamily::Cccs))
    .with(ir::Feature::Family(ir::ModelFamily::Mutual))
    .with(ir::Feature::Family(ir::ModelFamily::TlineIdeal))
    .with(ir::Feature::Family(ir::ModelFamily::Module))
    .with(ir::Feature::Family(ir::ModelFamily::Resistor))
    .with(ir::Feature::Family(ir::ModelFamily::Capacitor))
    .with(ir::Feature::Family(ir::ModelFamily::Inductor))
    .with(ir::Feature::Family(ir::ModelFamily::Diode))
    .with(ir::Feature::Family(ir::ModelFamily::Mos1));

/// A model of a shipped module whose file no read `load` names, or whose family the read files
/// load from files of different modules.
pub(super) const LIBRARY: ir::Code = ir::Code::new(b'P', 2101);
/// An instance parameter of the module given on a `model` line.
pub(super) const MISPLACED: ir::Code = ir::Code::new(b'P', 2102);
/// A parameter the module does not have.
pub(super) const UNKNOWN: ir::Code = ir::Code::new(b'P', 2103);
/// A `type=` other than the literal 1 or -1.
pub(super) const POLARITY: ir::Code = ir::Code::new(b'P', 2105);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        LIBRARY,
        ir::Info {
            title: "a model of a module no read load provides unambiguously",
            cite: "VD cir-loading, cir-masters; RULINGS R17, R41; DIALECT-VACASK run mod (one module per shipped file)",
            explain: "The IR names a model's implementation by its family and the design's device library of that family; a model of a shipped module whose file no read file loads, or whose family is loaded from files of two modules, has no such library, so the statement is kept as VACASK's text.",
            kind: ir::Kind::Note,
        },
    ),
    (
        MISPLACED,
        ir::Info {
            title: "an instance parameter given on a model line",
            cite: "VD cir-instance (Parameter override order); RULINGS R41 (Q4)",
            explain: "VACASK lets a model line give an OSDI instance parameter as the default of its instances; the IR's model parameters are the family's model parameters only, so the statement is kept as VACASK's text.",
            kind: ir::Kind::Note,
        },
    ),
    (
        UNKNOWN,
        ir::Info {
            title: "a parameter the module does not have",
            cite: "INV-semantics 7.11; DIALECT-VACASK run bp (\"Parameter 'dc' not found.\")",
            explain: "VACASK refuses a parameter its module lacks (and any parameter on a builtin's model line). The statement is kept as text and the deck refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        POLARITY,
        ir::Info {
            title: "a model type other than 1 or -1, or a diode level other than 1",
            cite: "INV-semantics 7.21; the shipped modules' `type` parameter (NMOS 1, PMOS -1)",
            explain: "A model's polarity is read from a literal `type=1` or `type=-1`, a diode's family from `level=1` (level 3 is not yet proven equal to ngspice's); an expression or another value is kept as VACASK's text.",
            kind: ir::Kind::Note,
        },
    ),
];

/// `model name module params`.
pub(super) fn model(l: &mut Lower<'_, '_, '_>, n: Node) {
    let fields = l.cst.ast.fields(n).to_vec();
    let invalid = |l: &mut Lower<'_, '_, '_>| {
        read::opaque(
            l,
            n,
            ir::OpaqueClass::InvalidInSource,
            Some(super::codes::PRIVATE),
        )
    };
    let [name, module, rest @ ..] = fields.as_slice() else {
        return invalid(l);
    };
    let name = read::field_name(l.cst, *name);
    let word = read::field_name(l.cst, *module);
    let shipped = families::module(&word).filter(|m| families::PROVEN.contains(&m.word));
    let family = shipped.map_or(ir::ModelFamily::Module, |m| m.family);
    let builtin = shipped.is_some_and(|m| m.file.is_empty());
    if shipped.is_some() && !builtin && library_word(l, family) != Some(word.as_str()) {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(LIBRARY));
    }
    let mut entries: Vec<(String, u32)> = Vec::with_capacity(rest.len());
    for &f in rest {
        let ast = &l.cst.ast;
        if ast.tag(Node(f)) != Tag::Assign {
            return invalid(l);
        }
        entries.push((read::field_name(l.cst, ast.lhs(Node(f))), ast.rhs(Node(f))));
    }
    if super::read_elements::repeated(&entries) {
        return read::opaque(
            l,
            n,
            ir::OpaqueClass::InvalidInSource,
            Some(super::read_elements::REPEATED),
        );
    }
    // Validate the whole statement before the first push (A1, A2).
    let mut polarity = match families::has_polarity(family) {
        true => ir::Polarity::N,
        false => ir::Polarity::None,
    };
    let mut keep: Vec<(String, u32)> = Vec::with_capacity(entries.len());
    let mut version = None;
    for (key, value) in entries {
        if builtin {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(UNKNOWN));
        }
        // A user module's parameters are its own names, as written (no rename).
        let ir_name = match families::model_name_in(&word, &key) {
            Name::Ir(_) if family == ir::ModelFamily::Module => key.clone(),
            Name::Ir(x) => x,
            Name::Misplaced => {
                return read::opaque(l, n, ir::OpaqueClass::Private, Some(MISPLACED));
            }
            Name::Unknown => {
                return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(UNKNOWN));
            }
        };
        if let Err(r) = expr_read::check_expr(l, value, Context::Param) {
            return read::opaque(l, n, r.class, Some(r.code));
        }
        match (ir_name.as_str(), polarity != ir::Polarity::None) {
            ("type", true) => match type_literal(l, value) {
                Some(p) => polarity = p,
                None => return read::opaque(l, n, ir::OpaqueClass::Private, Some(POLARITY)),
            },
            ("version", _) if family != ir::ModelFamily::Module => version = Some(value),
            // `level=1` is the family itself (sp_diode's default); level 3 is not proven yet.
            ("level", _) if family == ir::ModelFamily::Diode => {
                if type_literal(l, value) != Some(ir::Polarity::N) {
                    return read::opaque(l, n, ir::OpaqueClass::Private, Some(POLARITY));
                }
            }
            _ => keep.push((ir_name, value)),
        }
    }
    let id = l.b.name(&name);
    let design = l.b.design();
    let def = design.lookup(l.scope, ir::DefKind::Model, id);
    if !def.is_none() && design.def(def).scope == l.scope {
        return read::opaque(
            l,
            n,
            ir::OpaqueClass::InvalidInSource,
            Some(read_structure::REDEFINED),
        );
    }
    let mut params = Vec::with_capacity(keep.len());
    for (key, value) in &keep {
        let e = expr_read::expr(l, *value, Context::Param).expect("checked");
        params.push((key.as_str(), e));
    }
    let version = version.map_or(ir::ExprId::NONE, |v| {
        expr_read::expr(l, v, Context::Param).expect("checked")
    });
    let module = match family {
        ir::ModelFamily::Module => {
            let m = l.b.name(&word);
            let def = l.b.design().lookup(l.scope, ir::DefKind::Module, m);
            ir::Ref { name: m, def }
        }
        _ => ir::Ref::unresolved(ir::NameId::NONE),
    };
    let params = match params.is_empty() {
        true => ir::List::EMPTY,
        false => l.b.params(&params),
    };
    let row = ir::Model {
        name: id,
        params,
        bin_of: ir::NameId::NONE,
        ako: ir::Ref::unresolved(ir::NameId::NONE),
        module,
        version,
        word: ir::NameId::NONE,
        family,
        polarity,
    };
    let span = l.span(n);
    l.b.model(l.scope, row, span);
}

// ---- private ----

/// The module word the read files' loads give `family`: the module of every loaded file of
/// the family when they agree, else `None` (no load, or files of two modules).
fn library_word(l: &Lower<'_, '_, '_>, family: ir::ModelFamily) -> Option<&'static str> {
    let mut words = l
        .named_files
        .iter()
        .filter(|p| super::tables::library(p) == Some(family))
        .filter_map(|p| families::file_word(p));
    let first = words.next()?;
    words.all(|w| w == first).then_some(first)
}

/// The polarity a `type=` value states (and `level=1`'s 1 as N): the literal 1 (N) or -1 (P), integer or real (run t1:
/// `type=1.` reads as the integer 1).
fn type_literal(l: &Lower<'_, '_, '_>, value: u32) -> Option<ir::Polarity> {
    let ast = &l.cst.ast;
    let (negative, at) = match ast.tag(Node(value)) {
        Tag::Unary if l.cst.tokens.text(ast.main_token(Node(value))) == "-" => {
            (true, ast.lhs(Node(value)))
        }
        _ => (false, value),
    };
    if ast.tag(Node(at)) != Tag::Num {
        return None;
    }
    let number = super::number::lex(l.cst.tokens.text(ast.main_token(Node(at)))).ok()?;
    let lit = number.literal;
    // By value: `type=1.` is the integer 1 to VACASK (run t1); `eq_value` keeps Int apart.
    let one = lit.value() == 1.0;
    match (one, negative) {
        (true, false) => Some(ir::Polarity::N),
        (true, true) => Some(ir::Polarity::P),
        _ => None,
    }
}
