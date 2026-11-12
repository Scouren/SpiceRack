//! Pass `formal_defaults_to_instances`: a design in, the same design out with every instance
//! that omits the actual of a formal whose default is not a constant passing that default as
//! its actual. It removes `Extra::FormalNotConstant` (VACASK lets an instance override only a
//! formal whose default folds to a constant; A12 K2, A25).
//!
//! The formal's own default then (R13: no invented value): the row
//! `formal_defaults_to_instances` drops it and requires `Extra(FormalWithoutDefault)`, the exact
//! statement that every instance passes it; its twin `formal_defaults_to_instances_default`,
//! for a target without that feature, makes it the literal `0` and puts a `PASS_DEFAULT` note
//! at the subcircuit saying the default is the pass's and no instance reads it.
//!
//! ngspice's rule exactly (R53, A29 and its addendum): in the copy each instance gets, an
//! unbound parameter read is looked up from the instance's scope, a read of a sibling formal is
//! that instance's actual for it (or the sibling's default, expanded the same way), and a
//! function call stays bound to the definition's function.
//!
//! Left with a note: a default holding a draw (`DRAW_IN_DEFAULT`: one draw per copy would not
//! be the draw the default made), and a subcircuit an element names that no definition binds
//! (`UNKNOWN_INSTANCES`: an instance the pass cannot see would read the new default).
//!
//! Body pending `ir-kernel`'s per-root rewrite door (`rewrite_tree`, asked with usage code):
//! each copy's substitution depends on its instance, which `rewrite_exprs` cannot express.
//!
//! Cites: R53; `rewrite/AMEND-A12.md` A29, K2, A25; `rewrite/DIALECT-VACASK.md` §6.
//!
//! Contents: `pub fn formal_defaults_to_instances(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub fn formal_defaults_to_instances_default(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const DRAW_IN_DEFAULT: ir::Code`, `pub const UNKNOWN_INSTANCES: ir::Code`,
//! `pub const PASS_DEFAULT: ir::Code`

use crate::ir;

/// The note at a subcircuit whose non-constant default holds a draw.
pub const DRAW_IN_DEFAULT: ir::Code = ir::Code::new(b'U', 305);

/// The note at a subcircuit some element names without a binding.
pub const UNKNOWN_INSTANCES: ir::Code = ir::Code::new(b'U', 306);

/// The note at a subcircuit whose formals' replaced defaults are the pass's literal `0`.
pub const PASS_DEFAULT: ir::Code = ir::Code::new(b'U', 308);

/// Passes every non-constant default at each instance that omits it.
// ponytail: red until the per-root rewrite door lands; the fixtures state what it must do.
pub fn formal_defaults_to_instances(_design: &mut ir::Design, _diags: &mut ir::Diags) {}

/// As [`formal_defaults_to_instances`], the replaced defaults the literal `0`, with a
/// `PASS_DEFAULT` note.
// ponytail: red until the per-root rewrite door lands, as above.
pub fn formal_defaults_to_instances_default(_design: &mut ir::Design, _diags: &mut ir::Diags) {}
