//! What no feature states and this writer cannot spell (`Write::admits`): a name Spectre would
//! read as something else, and an option of another dialect. One walk over the reachable items.
//!
//! - a name is letters, digits, `_` and `!` (MAN: names; a `$` starts a built-in variable), not
//!   a statement keyword where it heads a statement (an instance or analysis named `model` reads
//!   as a model statement; CODE: the lexicon's head keywords);
//! - a parameter or function named `temp` or `tnom` reads back as the temperature, a function
//!   named like a built-in one (or `v`) as that one;
//! - an untyped instance parameter whose key is one of the element's own (`r`, `m`, `trise`, …,
//!   `write_elements::own_keys`), or a key with an upper-case letter on a primitive or a model
//!   (the reader refuses it: SFE-102); a model key `type`, `level` or `version`;
//! - a private option of another dialect;
//! - an AM whose modulation runs from the delay (Spectre's runs from the carrier phase), or
//!   whose offset and index are both other than 1 (RUN-SP P4-055);
//! - a function whose body reads the circuit (MAN: SR p.870, no `v()` in a body), and a MOSFET's
//!   area (Spectre's MOS has none), a line naming a model (`tline` has none here), an inductance that reads the circuit and a
//!   `bsource` temperature offset (no evidence);
//! - a subcircuit instance inside its own subcircuit's body (the reader's `RECURSION`);
//! - an element whose master no definition binds, of a kind the master decides (a device, a
//!   resistor of a model, a subcircuit instance): read back, nothing says what it is (unless a
//!   `model` statement kept as text declares it).
//!
//! Contents: `admits` (crate-private)

use super::codes;
use super::write_elements::{is_one, own_keys};
use crate::ir;

/// The statement keywords of the lexicon (CODE: `syntax/dialects/spectre.rs` KEYWORDS).
const KEYWORDS: &[&str] = &[
    "simulator",
    "include",
    "ahdl_include",
    "library",
    "endlibrary",
    "section",
    "endsection",
    "inline",
    "subckt",
    "ends",
    "model",
    "parameters",
    "statistics",
    "process",
    "mismatch",
    "vary",
    "correlate",
    "truncate",
    "if",
    "else",
    "real",
    "return",
    "global",
    "save",
    "ic",
    "nodeset",
];

/// The built-in functions the reader knows (`expr_read.rs`), and `v`.
const BUILTINS: &[&str] = &[
    "abs", "sqrt", "exp", "log", "log10", "pow", "min", "max", "floor", "ceil", "int", "round",
    "fmod", "hypot", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh",
    "acosh", "atanh", "atan2", "ln", "ddt", "idt", "v", "i",
];

/// Pushes `codes::UNSPELLED` at each item of `design` this writer cannot spell.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    use ir::ItemKind as K;
    let d = design;
    let kept = kept_models(d);
    for (at, scope) in d.reachable() {
        let ok = match at.kind {
            K::Element => {
                let row: ir::Element = d.row(at.index);
                element(d, &row, &kept) && !inside_master(d, &row, scope)
            }
            K::Model => {
                let row: ir::Model = d.row(at.index);
                head(d.str(row.name))
                    && d.params(row.params).iter().all(|&(k, _)| {
                        let k = d.str(k);
                        name(k) && lower(k) && !matches!(k, "type" | "level" | "version")
                    })
            }
            K::Subckt => {
                let row: ir::item::Subckt = d.row(at.index);
                head(d.str(row.name))
                    && d.names(row.ports).iter().all(|&p| name(d.str(p)))
                    && d.params(row.params).iter().all(|&(k, _)| param(d.str(k)))
            }
            K::Param => param(d.str(d.row::<ir::item::Param>(at.index).name)),
            K::Func => {
                let row: ir::item::Func = d.row(at.index);
                let n = d.str(row.name);
                param(n)
                    && !BUILTINS.contains(&n)
                    && d.names(row.args).iter().all(|&a| param(d.str(a)))
                    && !reads_circuit(d, row.body)
            }
            // an unnamed one is a feature's (AnalysisUnnamed, LoopUnnamed), not this check's
            K::Analysis => named(d, d.row::<ir::Analysis>(at.index).name),
            K::Loop => named(d, d.row::<ir::Loop>(at.index).name),
            K::Option => {
                let row: ir::OptionRow = d.row(at.index);
                row.key != ir::OptionKey::Private
                    || (d.str(row.dialect) == "spectre"
                        && name(d.str(row.private))
                        && lower(d.str(row.private)))
            }
            K::Save => {
                let row: ir::Save = d.row(at.index);
                d.probes(row.probes)
                    .iter()
                    .all(|p| d.names(p.a).iter().all(|&n| name(d.str(n))))
            }
            K::Statistics => {
                let row: ir::Statistics = d.row(at.index);
                [row.process, row.mismatch]
                    .iter()
                    .flat_map(|&l| d.varies(l))
                    .all(|v| param(d.str(v.param.name)))
            }
            _ => true,
        };
        if !ok {
            diags.push_at(codes::UNSPELLED, d, at, 0);
        }
    }
}

// ---- private ----

/// Whether `s` is a name Spectre reads as itself: letters, digits, `_`, `!`, not empty.
fn name(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'!')
}

/// Whether `id` is no name, or one that heads a statement.
fn named(d: &ir::Design, id: ir::NameId) -> bool {
    id.is_none() || head(d.str(id))
}

/// A name that heads a statement: a `name` and no keyword.
fn head(s: &str) -> bool {
    name(s) && !KEYWORDS.contains(&s)
}

/// A parameter's name: a `head` that is not a temperature.
fn param(s: &str) -> bool {
    head(s) && !matches!(s, "temp" | "tnom")
}

/// Whether `s` holds no upper-case letter.
fn lower(s: &str) -> bool {
    !s.bytes().any(|b| b.is_ascii_uppercase())
}

/// Whether subcircuit instance `row`, listed in `scope`, stands in its own master's body,
/// directly or in its `if` branches (the reader refuses it: `codes::RECURSION`).
fn inside_master(d: &ir::Design, row: &ir::Element, scope: ir::ScopeId) -> bool {
    if row.kind != ir::ElemKind::Instance || row.model.def.is_none() {
        return false;
    }
    let item = d.def(row.model.def).item;
    let mut at = scope;
    // Bounded: each step moves to the enclosing scope, and the scope tree is acyclic.
    while !at.is_none() {
        let s = d.scope(at);
        if s.owner == item {
            return true;
        }
        if s.kind != ir::ScopeKind::Branch {
            return false;
        }
        at = s.parent;
    }
    false
}

/// Whether wave `w` is no AM, or one Spectre's `type=sine` spells.
fn am_ok(d: &ir::Design, w: ir::WaveRef) -> bool {
    if w.is_none() {
        return true;
    }
    match d.wave(w) {
        ir::Wave::Am(a) => {
            a.mod_time == ir::wave::AmModTime::CarrierPhase
                && (is_one(d, a.mod_offset) || is_one(d, a.mod_index))
        }
        _ => true,
    }
}

/// Whether the tree under `e` reads the circuit.
fn reads_circuit(d: &ir::Design, e: ir::ExprId) -> bool {
    let mut todo = vec![e];
    let mut kids = Vec::new();
    // Bounded: an expression tree is acyclic and finite.
    while let Some(x) = todo.pop() {
        if matches!(
            d.expr(x),
            ir::Expr::V(..) | ir::Expr::I(_) | ir::Expr::Time | ir::Expr::Freq
        ) {
            return true;
        }
        kids.clear();
        d.children(x, &mut kids);
        todo.extend_from_slice(&kids);
    }
    false
}

/// The names the `model` statements kept as Spectre text declare: an instance of one reads
/// back as that model's kind, though no IR definition binds it.
fn kept_models(d: &ir::Design) -> Vec<String> {
    d.reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Opaque)
        .filter_map(|(i, _)| {
            let text = d.text(d.row::<ir::item::Opaque>(i.index).text);
            let mut words = text.split_whitespace();
            (words.next() == Some("model")).then(|| words.next().map(str::to_string))?
        })
        .collect()
}

/// An element's names and untyped keys.
fn element(d: &ir::Design, row: &ir::Element, kept: &[String]) -> bool {
    use ir::ElemKind as K;
    let primitive = !matches!(row.kind, K::Instance | K::Unresolved);
    let own = own_keys(row.kind);
    let keys = d.params(row.params).iter().all(|&(k, _)| {
        let k = d.str(k);
        name(k) && !own.contains(&k) && (!primitive || lower(k))
    });
    let sense = match row.kind {
        K::Cccs | K::Ccvs => name(d.str(d.controlled(row).sense)),
        _ => true,
    };
    let typed_by_master = !matches!(row.kind, K::Unresolved) && !row.model.name.is_none();
    let area = row.kind != K::Mosfet || d.semi(row).area.is_none();
    // an AM Spectre spells: its modulation timed from the carrier phase, and an offset of 1 or
    // an index of 1 (RUN-SP P4-055)
    let am = !matches!(row.kind, K::VSource | K::ISource) || am_ok(d, d.source(row).wave);
    // a bsource's temperature offset: no run settled a `trise` there
    let behav = !matches!(row.kind, K::Behav | K::ChargeCapacitor) || d.behav(row).dtemp.is_none();
    // an inductance that reads the circuit: no run settled Spectre's `l=` there
    let inductance = row.kind != K::Inductor
        || d.passive(row).value.is_none()
        || !reads_circuit(d, d.passive(row).value);
    let bound = || !row.model.def.is_none() || kept.iter().any(|k| k == d.str(row.model.name));
    (!typed_by_master || bound())
        && area
        && (row.kind != K::LosslessLine || row.model.name.is_none())
        && am
        && behav
        && inductance
        && head(d.str(row.name))
        && d.names(row.terminals).iter().all(|&t| name(d.str(t)))
        && (row.model.name.is_none() || head(d.str(row.model.name)))
        && keys
        && sense
}
