//! Pass `splice_elaborate`: a design in, the same design out with a deck's one `Elaborate`,
//! standing before every run item, replaced by its circuits: each named subcircuit's body items
//! moved into the deck's top scope where the subcircuit stood (their definitions with them),
//! its formals made top-level parameters of their defaults, the subcircuit item gone. It
//! removes `Item(Elaborate)` (ngspice's top circuit is its deck's top scope; VACASK's is what
//! `elaborate` names, A12 R1) and requires `Item(Param)` (the formals).
//!
//! Left with a note (`ELABORATE_KEPT`): a second `Elaborate` (ngspice cannot swap its top
//! circuit), one after a run item, a top scope that holds elements of its own, a named
//! subcircuit some element instantiates or with a formal without a default, and a name two
//! spliced bodies (or a body and the top) both define.
//!
//! Body pending `ir-kernel`'s item-move door (A32 follow-up): moved items' definitions must be
//! re-keyed to the top scope.
//!
//! Cites: `rewrite/AMEND-A12.md` R1; `rewrite/DIALECT-VACASK.md` §6 (VS V1, V2).
//!
//! Contents: `pub fn splice_elaborate(design: &mut ir::Design, diags: &mut ir::Diags)`,
//! `pub const ELABORATE_KEPT: ir::Code`

use crate::ir;

/// The note at an `Elaborate` the pass cannot splice exactly.
pub const ELABORATE_KEPT: ir::Code = ir::Code::new(b'U', 309);

/// Splices a deck's one leading `Elaborate` into its top scope.
// ponytail: red until the move door lands; the fixtures state what it must do.
pub fn splice_elaborate(_design: &mut ir::Design, _diags: &mut ir::Diags) {}
