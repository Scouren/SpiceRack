//! The top-level instance (R70, A43): VACASK's `__topinst__` is the design itself, so
//! `alter instance("__topinst__") a=2` and `sweep … instance="__topinst__" parameter="a"` set
//! the top-level parameter `a` (settle run/top_alter, top_sweep), and one no top level defines
//! is VACASK's error (top_nosuch: "Parameter 'zz' not found.").
//!
//! Contents: `TOP_INSTANCE`, `top_param`, `bind_top_param` (crate-private)

use crate::dialect::vacask::read::Lower;
use crate::ir;

/// The name of the top-level instance: the design itself (R70; VD cmd-sweep, cmd-alter).
pub(in crate::dialect::vacask) const TOP_INSTANCE: &str = "__topinst__";

/// Whether `name` is a parameter of the file's top level, as the design holds it so far.
pub(in crate::dialect::vacask) fn top_param(l: &Lower<'_, '_, '_>, name: &str) -> bool {
    let design = l.b.design();
    let id = design.find_name(name);
    let top = design.file(l.cst.file).top;
    !id.is_none() && !design.lookup(top, ir::DefKind::Param, id).is_none()
}

/// Parameter target `t` bound to the file's top-level parameter of its name.
pub(in crate::dialect::vacask) fn bind_top_param(
    l: &Lower<'_, '_, '_>,
    mut t: ir::Target,
) -> ir::Target {
    let design = l.b.design();
    let top = design.file(l.cst.file).top;
    t.a.def = design.lookup(top, ir::DefKind::Param, t.a.name);
    debug_assert!(!t.a.def.is_none(), "checked by top_param");
    t
}
