//! Pass `hoist_nested_subckts` (rows `hoist_nested_subckts` and `hoist_enclosing_refs`): a
//! design in, the same design out with every subcircuit listed in another subcircuit's body
//! moved one level out per step (into the scope listing its enclosing subcircuit, right after
//! it) until none is nested. It removes `Placed(Subckt, Subckt)` and, with it,
//! `Extra::EnclosingRef` (VACASK resolves a name in the own subcircuit, then the top, never an
//! enclosing subcircuit; A12 K2). A read of an enclosing subcircuit's parameter becomes a new
//! formal of the same name (default `0`: every instance passes it), and each instance passes
//! that name, bound to the enclosing definition; a name taken where the subcircuit lands is
//! renamed (`<name>_<n>`), its instances following. Requires `Extra(FormalNotConstant)`? No:
//! the new formals' defaults are constants; it requires nothing.
//!
//! Left with a note: a subcircuit whose body reads an enclosing model, function or subcircuit
//! (`READS_ENCLOSING`), or one defined in a branch, loop or section inside a subcircuit
//! (`NOT_IN_BODY`).
//!
//! Body pending `ir-kernel`'s item-move door (A32 follow-up): the definitions of a moved item
//! must be re-keyed to its new scope, which no door does today.
//!
//! Cites: `rewrite/AMEND-A12.md` K2, A15 K1, A32; R27; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub fn hoist_nested_subckts(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const READS_ENCLOSING: ir::Code`, `pub const NOT_IN_BODY: ir::Code`

use crate::ir;

/// The note at a nested subcircuit whose body reads an enclosing model, function or subcircuit.
pub const READS_ENCLOSING: ir::Code = ir::Code::new(b'U', 303);

/// The note at a subcircuit defined in a branch, loop or section inside a subcircuit.
pub const NOT_IN_BODY: ir::Code = ir::Code::new(b'U', 304);

/// Hoists every nested subcircuit one level per step until none is nested.
// ponytail: red until the move door lands (`Editor::move_item`); the fixtures and
// `tests/passes_structure.rs` state what it must do.
pub fn hoist_nested_subckts(_design: &mut ir::Design, _diags: &mut ir::Diags) {}
