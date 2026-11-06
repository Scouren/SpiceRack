//! The registry's bound (`DIALECT-PLUGIN.md` §7.2): `LANGS_MAX`
//! languages, the built-ins among them; one more is `RegisterError::Full`
//! and registers nothing. Its own test binary: it fills the process's
//! registry, which every other test shares.
//!
//! Oracle: the bound as declared (`lang::LANGS_MAX`), counted by name.
//!
//! Contents: none (tests only).

use spicerack::syntax::Lang;
use spicerack::syntax::def::{DialectDef, RegisterError, register};
use spicerack::syntax::lang::LANGS_MAX;

// ---- private ----

#[test]
fn the_registry_holds_langs_max_languages() {
    let plugin = |i: usize| DialectDef::new(&format!("fill-{i}")).extends(Lang::NGSPICE);
    let room = LANGS_MAX - Lang::BUILTIN_COUNT;
    for i in 0..room {
        register(plugin(i)).expect("room for it");
    }
    assert_eq!(register(plugin(room)), Err(RegisterError::Full));
    assert_eq!(Lang::from_name(&format!("fill-{room}")), None);
    // A held name is still found: re-registering it is no new row.
    assert!(register(plugin(0)).is_ok());
}
