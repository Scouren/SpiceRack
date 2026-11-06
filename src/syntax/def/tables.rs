//! The definition's list rows ([`DialectDef`]): the Spectre analyses a
//! target has, the language's temperature word, builtin constants and
//! built-in device names, each appended by one call.
//!
//! Cites: none (each definition cites its manual where it declares a row).
//!
//! Contents:
//! - `DialectDef::temp_name(self, word: &str) -> Self`, `DialectDef::builtin_constant(self, name: &str, value: &str) -> Self`
//! - `DialectDef::builtin_device(self, name: &str) -> Self`, `DialectDef::spectre_analysis(self, kind: &str) -> Self`
//! - `DialectDef::vacask_analysis(self, kind: &str) -> Self`

use super::{DialectDef, leak};

impl DialectDef {
    /// The expression word for the circuit temperature.
    pub fn temp_name(mut self, word: &str) -> Self {
        self.b.temp_name = leak(word);
        self
    }

    /// A builtin constant of the language and its value's text.
    pub fn builtin_constant(mut self, name: &str, value: &str) -> Self {
        let mut v = self.b.builtin_constants.to_vec();
        v.push((leak(name), leak(value)));
        self.b.builtin_constants = Box::leak(v.into_boxed_slice());
        self
    }

    /// A built-in device name of the language (a model type that names it
    /// is the built-in).
    pub fn builtin_device(mut self, name: &str) -> Self {
        let mut v = self.b.builtin_devices.to_vec();
        if !v.contains(&name) {
            v.push(leak(name));
        }
        self.b.builtin_devices = Box::leak(v.into_boxed_slice());
        self
    }

    /// The target has an analysis for Spectre analysis `kind`.
    pub fn spectre_analysis(mut self, kind: &str) -> Self {
        let mut v = self.b.spectre_analyses.to_vec();
        if !v.contains(&kind) {
            v.push(leak(kind));
        }
        self.b.spectre_analyses = Box::leak(v.into_boxed_slice());
        self
    }

    /// The target has an analysis for VACASK analysis `kind`, untranslated.
    pub fn vacask_analysis(mut self, kind: &str) -> Self {
        let mut v = self.b.vacask_analyses.to_vec();
        if !v.contains(&kind) {
            v.push(leak(kind));
        }
        self.b.vacask_analyses = Box::leak(v.into_boxed_slice());
        self
    }
}
