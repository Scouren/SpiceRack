//! Pass `device_libraries`: a design in, the same design out with one `DeviceLibrary { family,
//! path absent }` for every family a reachable model uses that needs a library and has no
//! reachable one, at the top of the entry file after its title, in the order the models first
//! appear. It removes `CircuitExtra::ModelWithoutLibrary` (design-level) and requires
//! `Item(DeviceLibrary)`; the family is the model's already (`Family(f)`, A25). Nothing else
//! moves, and it always converts: no note.
//!
//! Cites: `rewrite/IR-CIRCUIT.md` §4a; AMEND-A12 C5; R17, R44, R45.
//!
//! Contents: `pub fn device_libraries(design: &mut ir::Design, diags: &mut ir::Diags)`

use super::libraries::add_libraries;
use crate::ir;

/// Adds the libraries the reachable models' families lack. Adds no diagnostic.
pub fn device_libraries(design: &mut ir::Design, _diags: &mut ir::Diags) {
    let families: Vec<ir::ModelFamily> = design
        .reachable()
        .into_iter()
        .filter(|(i, _)| i.kind == ir::ItemKind::Model)
        .map(|(i, _)| design.row::<ir::Model>(i.index).family)
        .collect();
    add_libraries(design, &families);
}
