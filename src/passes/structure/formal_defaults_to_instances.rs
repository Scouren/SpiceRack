//! Pass `formal_defaults_to_instances`: a design in, the same design out with every instance
//! that omits the actual of a formal whose default is not a constant passing that default as
//! its actual, and the formal's own default the literal `0` (no instance reads it any more). It
//! removes `Extra::FormalNotConstant` (VACASK lets an instance override only a formal whose
//! default folds to a constant; A12 K2, A25) and requires nothing.
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
//! `pub const DRAW_IN_DEFAULT: ir::Code`, `pub const UNKNOWN_INSTANCES: ir::Code`

use crate::ir;

/// The note at a subcircuit whose non-constant default holds a draw.
pub const DRAW_IN_DEFAULT: ir::Code = ir::Code::new(b'U', 305);

/// The note at a subcircuit some element names without a binding.
pub const UNKNOWN_INSTANCES: ir::Code = ir::Code::new(b'U', 306);

/// Passes every non-constant default at each instance that omits it.
// ponytail: red until the per-root rewrite door lands; the fixtures state what it must do.
pub fn formal_defaults_to_instances(_design: &mut ir::Design, _diags: &mut ir::Diags) {}
