//! The definition's list rows ([`DialectDef`]): the Spectre analyses a
//! target has, the language's temperature word, builtin constants and
//! built-in device names, the SPICE options it maps, the master a SPICE
//! `.model` type gets, the reader's own words (kept constructs, dot
//! rules), each appended by one call.
//!
//! Cites: none (each definition cites its manual where it declares a row).
//!
//! Contents:
//! - `DialectDef::temp_name(self, word: &str) -> Self`, `DialectDef::builtin_constant(self, name: &str, value: &str) -> Self`
//! - `DialectDef::builtin_device(self, name: &str) -> Self`, `DialectDef::spectre_analysis(self, kind: &str) -> Self`
//! - `DialectDef::vacask_analysis(self, kind: &str) -> Self`
//! - `DialectDef::option_to(self, spice: &str, own: &str) -> Self`, `DialectDef::option_method(self, spice: &str) -> Self`,
//!   `DialectDef::option_cosmetic(self, spice: &str) -> Self`
//! - `DialectDef::model_master(self, kind: &str, levels: &[u32], master: &str, type_: Option<&str>) -> Self`,
//!   `DialectDef::spice_model_type(self, kind: &str) -> Self`
//! - `DialectDef::kept_word(self, word: &str, c: Construct) -> Self`, `DialectDef::dot_rule(self, word: &str, r: DotRule) -> Self`

use super::{DialectDef, leak, put};
use crate::syntax::behaviour::{DotRule, ModelMaster, OptionTo};
use crate::syntax::render::capability::Construct;

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

    /// SPICE option `spice` is the target's option `own`, of the same
    /// quantity: `.options VNTOL=1u` is Spectre `vabstol=1u`.
    pub fn option_to(self, spice: &str, own: &str) -> Self {
        self.option_row(spice, OptionTo::Name(leak(own)))
    }

    /// SPICE option `spice` is the integration method, its value spelled
    /// by the writer (VACASK `tran_method`).
    pub fn option_method(self, spice: &str) -> Self {
        self.option_row(spice, OptionTo::Method)
    }

    /// SPICE option `spice` changes no number in the result (`acct`,
    /// `nopage`): left out, noted W0024.
    pub fn option_cosmetic(self, spice: &str) -> Self {
        self.option_row(spice, OptionTo::Cosmetic)
    }

    /// Sets `spice`'s (lowercased) row of `option_map`: in place where it
    /// holds the key, else last.
    fn option_row(mut self, spice: &str, to: OptionTo) -> Self {
        let mut v = self.b.option_map.to_vec();
        put(&mut v, leak(&spice.to_ascii_lowercase()), to);
        self.b.option_map = Box::leak(v.into_boxed_slice());
        self
    }

    /// SPICE `.model` type `kind` at any of `levels` is the target's master
    /// `master`, written with `type=type_` (Spectre `bsim4 type=n`, VACASK
    /// `sp_bsim4v8 type=-1`). A row of the same type and LEVELs is replaced
    /// in place. The type is a SPICE one only once `spice_model_type`
    /// names it.
    pub fn model_master(
        mut self,
        kind: &str,
        levels: &[u32],
        master: &str,
        type_: Option<&str>,
    ) -> Self {
        let row = ModelMaster {
            kind: leak(&kind.to_ascii_lowercase()),
            levels: Box::leak(levels.to_vec().into_boxed_slice()),
            master: leak(master),
            type_: type_.map(leak),
        };
        let mut v = self.b.model_masters.to_vec();
        match v
            .iter_mut()
            .find(|m| m.kind == row.kind && m.levels == row.levels)
        {
            Some(m) => *m = row,
            None => v.push(row),
        }
        self.b.model_masters = Box::leak(v.into_boxed_slice());
        self
    }

    /// The target reads `.model` type `kind` as SPICE's: at a LEVEL no
    /// `model_master` row covers it is refused (W0104), where a type not
    /// named is written as a master of the target's own.
    pub fn spice_model_type(mut self, kind: &str) -> Self {
        let kind = kind.to_ascii_lowercase();
        let mut v = self.b.spice_model_types.to_vec();
        if !v.contains(&kind.as_str()) {
            v.push(leak(&kind));
        }
        self.b.spice_model_types = Box::leak(v.into_boxed_slice());
        self
    }

    /// A line kept as this language's text whose first word is `word` is
    /// construct `c` ([`Behaviours::kept_words`](crate::syntax::behaviour::Behaviours::kept_words)):
    /// `word` lowercased, a dot card's with its dot (`.stateye`), an
    /// element's whole (`yneuron`). A one-letter key matches every element
    /// line of that letter; the reader adds the construct's own shape check
    /// (HSPICE's W line: a W model key), none for any other construct (`"a"`
    /// matches every A line). A key held is replaced in place.
    pub fn kept_word(mut self, word: &str, c: Construct) -> Self {
        let mut v = self.b.kept_words.to_vec();
        put(&mut v, leak(&word.to_ascii_lowercase()), c);
        self.b.kept_words = Box::leak(v.into_boxed_slice());
        self
    }

    /// Dot card `word` is read by rule `r`; the key is `word` lowercased
    /// with a leading dot dropped, so `.LIN` and `lin` are one key. It is
    /// read ahead of the shared dot cards ([`Behaviours::dot_rules`](crate::syntax::behaviour::Behaviours::dot_rules)).
    /// A key held is replaced in place; `register` refuses a `Note` whose
    /// code is no note.
    pub fn dot_rule(mut self, word: &str, r: DotRule) -> Self {
        let word = word.to_ascii_lowercase();
        let word = word.strip_prefix('.').unwrap_or(&word);
        let mut v = self.b.dot_rules.to_vec();
        put(&mut v, leak(word), r);
        self.b.dot_rules = Box::leak(v.into_boxed_slice());
        self
    }
}
