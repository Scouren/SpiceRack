//! Pass `hoist_nested_subckts` (rows `hoist_nested_subckts` and `hoist_enclosing_refs`): a
//! design in, the same design out with every subcircuit listed in another subcircuit's body
//! moved one level out per step (into the scope listing its enclosing subcircuit, right after
//! it) until none is nested. It removes `Placed(Subckt, Subckt)` and, with it,
//! `Extra::EnclosingRef` (VACASK resolves a name in the own subcircuit, then the top, never an
//! enclosing subcircuit; A12 K2). A read of an enclosing subcircuit's parameter becomes a new
//! formal of the same name, and each instance passes that name, bound to the enclosing
//! definition; a name taken where the subcircuit lands is renamed (`<name>_<n>`), its
//! instances following.
//!
//! The added formal's default (R13: no invented value): the rows `hoist_nested_subckts` and
//! `hoist_enclosing_refs` leave it without one and require `Extra(FormalWithoutDefault)`, the
//! exact statement that every instance passes it; their `_default` twins, for a target without
//! that feature, give it the literal `0` and put a `HOIST_DEFAULT` note at the subcircuit
//! saying the default is the hoist's and no instance reads it. `legalize` takes the first
//! usable row, so a target that can say "no default" gets the clean form.
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
//! `pub fn hoist_nested_subckts_default(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const READS_ENCLOSING: ir::Code`, `pub const NOT_IN_BODY: ir::Code`,
//! `pub const HOIST_DEFAULT: ir::Code`

use crate::ir;

/// The note at a nested subcircuit whose body reads an enclosing model, function or subcircuit.
pub const READS_ENCLOSING: ir::Code = ir::Code::new(b'U', 303);

/// The note at a subcircuit defined in a branch, loop or section inside a subcircuit.
pub const NOT_IN_BODY: ir::Code = ir::Code::new(b'U', 304);

/// The note at a hoisted subcircuit whose added formals got the literal default `0`.
pub const HOIST_DEFAULT: ir::Code = ir::Code::new(b'U', 307);

/// Hoists every nested subcircuit one level per step until none is nested; added formals have
/// no default.
// ponytail: red until the move door lands (`Editor::move_item`); the fixtures and
// `tests/passes_structure.rs` state what it must do.
pub fn hoist_nested_subckts(_design: &mut ir::Design, _diags: &mut ir::Diags) {}

/// As [`hoist_nested_subckts`], the added formals defaulting to `0`, with a `HOIST_DEFAULT` note.
// ponytail: red until the move door lands, as above.
pub fn hoist_nested_subckts_default(_design: &mut ir::Design, _diags: &mut ir::Diags) {}
