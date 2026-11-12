//! The structure statements → IR rows: the title, `include` (and `section=`), `section`,
//! `load`, `embed`, `subckt` (nested too) with its `parameters` split into formals and locals,
//! `parameters`, `global`, `ground`, `@if` chains.
//!
//! Rules (`rewrite/DIALECT-VACASK.md` §3.2, each with its evidence there):
//! - a subckt's `parameters` entry whose value folds to a constant at parse time (literals,
//!   builtin constants, functions of them) is a formal; one that reads a name is a local
//!   (RUN c1, t2);
//! - a parameter defined twice in one scope is refused (RUN n3, `R2001`);
//! - `@if` bodies hold instances, models and `@if`s: a `parameters` there is refused (`P2005`);
//! - `ground` names are node `0` everywhere; the statement itself is `Inert` (R15);
//! - a `load` of a library file VACASK ships is the device library (A12 C5, `a12.rs`); any
//!   other `.osdi` or `.va` a `Load`; a `map=` other than the default `Private`;
//! - an `embed` that a `load` or `include` names is `Private`, else `Inert` (C A20).
//!
//! Cites: VD cir-subckt, cir-subckt-nested, cir-conditional, cir-nodes, cir-loading,
//! input-embed, input-include.
//!
//! Contents: `FEATURES`, `CODES`, `REDEFINED`, `IN_BRANCH`, `ENCLOSING`, and the area functions
//! `title`, `include`, `section`, `subckt`, `if_chain`, `param`, `global`, `ground`, `load`,
//! `embed` (crate-private)

use super::read::Lower;
use crate::ir;
use crate::syntax::ast::Node;

/// What this area's rows use.
pub(super) const FEATURES: ir::FeatureSet = ir::FeatureSet::EMPTY
    .with(ir::Feature::Item(ir::ItemKind::Title))
    .with(ir::Feature::Item(ir::ItemKind::Comment))
    .with(ir::Feature::Item(ir::ItemKind::Include))
    .with(ir::Feature::Item(ir::ItemKind::Global))
    .with(ir::Feature::Item(ir::ItemKind::Load))
    .with(ir::Feature::Item(ir::ItemKind::Opaque))
    .with(ir::Feature::Item(ir::ItemKind::Subckt))
    .with(ir::Feature::Item(ir::ItemKind::Section))
    .with(ir::Feature::Item(ir::ItemKind::If))
    .with(ir::Feature::Item(ir::ItemKind::Param))
    .with(ir::Feature::Extra(ir::Extra::NestedSubckt))
    .with(ir::Feature::Extra(ir::Extra::IncludeSection))
    .with(ir::Feature::Extra(ir::Extra::CaseSensitiveNames));

/// A parameter defined twice in one scope.
pub(super) const REDEFINED: ir::Code = ir::Code::new(b'R', 2001);
/// A `parameters` statement inside an `@if` branch.
pub(super) const IN_BRANCH: ir::Code = ir::Code::new(b'P', 2005);
/// A name only an enclosing subcircuit defines.
pub(super) const ENCLOSING: ir::Code = ir::Code::new(b'R', 2003);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[
    (
        REDEFINED,
        ir::Info {
            title: "a parameter defined twice in one scope",
            cite: "DIALECT-VACASK Appendix A run n3 (\"Parameter redefinition.\"); INV-semantics 3.01",
            explain: "VACASK refuses a second definition of a parameter in one scope; the first is kept and the deck is refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        ENCLOSING,
        ir::Info {
            title: "a name only an enclosing subcircuit defines",
            cite: "DIALECT-VACASK Appendix A runs n1, m1; VD cir-hier (Scope)",
            explain: "VACASK looks a name up in the own subcircuit, then the top level, never in an enclosing subcircuit (\"Variable or constant … not defined\", \"Master … not found\"). The reference is left unbound and the deck refused.",
            kind: ir::Kind::Refusal,
        },
    ),
    (
        IN_BRANCH,
        ir::Info {
            title: "parameters inside an @if branch",
            cite: "VD cir-conditional (Allowed contents)",
            explain: "An @if branch holds instances, models and nested @if blocks only; VACASK rejects a parameters statement there.",
            kind: ir::Kind::Refusal,
        },
    ),
];

/// The title line of an entry deck (VD; S 14.12): a `Title` item; an empty line has no item.
pub(super) fn title(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: title")
}

/// `include "path" [section=name]`: an `Include` of the file `src` holds at that path
/// (resolved against the including file), else of the path as written.
pub(super) fn include(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: include")
}

/// `section name` … `endsection`: a `Section` and its body.
pub(super) fn section(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: section")
}

/// `subckt name (ports)` … `ends`: a `Subckt` whose head holds the constant `parameters`
/// entries of its body (in order), then the body with the other entries as local `Param`s.
pub(super) fn subckt(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: subckt")
}

/// `@if c` … `@elseif c` … `@else` … `@end`: one `If` with a branch per condition.
pub(super) fn if_chain(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: if_chain")
}

/// `parameters a=e b=e …` outside a subcircuit's head: one `Param` per entry, in order.
pub(super) fn param(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: param")
}

/// `global a b …` / `global (a b …)`: a `Global` of the names that are not ground; only ground
/// names: `Inert`.
pub(super) fn global(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: global")
}

/// `ground a b …`: `Inert` (its names became `0` everywhere through `Lower::grounds`).
pub(super) fn ground(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: ground")
}

/// `load "path" [map=…]`.
pub(super) fn load(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: load")
}

/// `embed "file" <<<MARK … >>>MARK`.
pub(super) fn embed(l: &mut Lower<'_, '_, '_>, n: Node) {
    let _ = (l, n);
    todo!("S4.0: embed")
}
