//! Pass `constant_param_to_formal`: a design in, the same design out with every `Param` listed
//! directly in a subcircuit's body whose value is a constant made a formal of that subcircuit,
//! its value the default, appended to the head in body order. It removes
//! `Extra::SubcktConstantParam` (VACASK reads `parameters x=1` in a body as a formal, ngspice's
//! `.param` there is none: A25, R48) and requires nothing: the default is a constant.
//!
//! Exact while no instance passes an actual of that name (none can override a local today);
//! a subcircuit an instance passes such a name to keeps the parameter with a `PASSED_LOCAL`
//! note, since the formal would take the actual.
//!
//! Body pending `ir-kernel`'s item-removal door (A32 follow-up): the parameter's definition
//! must leave the index before the formal's is defined.
//!
//! Cites: `rewrite/AMEND-A12.md` A25; R48.
//!
//! Contents: `pub fn constant_param_to_formal(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const PASSED_LOCAL: ir::Code`

use crate::ir;

/// The note at a subcircuit-body parameter an instance passes an actual for.
pub const PASSED_LOCAL: ir::Code = ir::Code::new(b'U', 310);

/// Makes each constant body parameter a formal of its subcircuit.
// ponytail: red until the removal door lands; the fixtures state what it must do.
pub fn constant_param_to_formal(_design: &mut ir::Design, _diags: &mut ir::Diags) {}
