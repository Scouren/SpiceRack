//! The `.model` card an instance uses, wherever the kit read it: this
//! file's, or another kit file's (its text parsed again), so a writer can
//! judge the card at the instance that elaborates it; and a card's
//! `level`, the number its source dialect gives the model.
//!
//! Cites: none (the kit's bindings, `resolve`).
//!
//! Contents:
//! - `Rd::instance_model(&self, n: u32, master: &str) -> Option<(Option<FileId>, u32)>`
//! - `Rd::kit(&self) -> Option<&Kit>`
//! - `Rd::instance_params(&self, n: u32, master: &str) -> Option<Vec<(String, String)>>`
//! - `pub(in crate::syntax::render) fn file_models<R>(kit: &Kit, file: FileId, f: impl FnMut(u32, Model) -> R) -> Vec<(u32, R)>`
//! - `Model::level(&self) -> u32`

use super::{Model, Rd};
use crate::syntax::ast::{NONE, Node};
use crate::syntax::resolve::{Binding, DefKind, FileId, Kit};
use crate::syntax::{parse, tokenize};

impl<'a> Rd<'a> {
    /// The `.model` card instance `n` (master word `master`) uses: the
    /// kit's binding (`None` file: this one), or with no kit this file's
    /// card of that name. `None`: no model card (a primitive, a subcircuit,
    /// an unknown master).
    pub(in crate::syntax::render) fn instance_model(
        &self,
        n: u32,
        master: &str,
    ) -> Option<(Option<FileId>, u32)> {
        let Some((kit, f)) = self.kit else {
            let card = *self.models.get(&master.to_ascii_lowercase())?;
            return Some((None, card));
        };
        let Binding::Def(id) = kit.binding(f, Node(n))? else {
            return None;
        };
        let def = kit.definition(id);
        if !matches!(def.kind, DefKind::Model { .. }) {
            return None;
        }
        let file = (def.file != f).then_some(def.file);
        Some((file, def.node.0))
    }

    /// The kit this file was read in.
    pub(in crate::syntax::render) fn kit(&self) -> Option<&'a Kit> {
        self.kit.map(|(k, _)| k)
    }

    /// The parameters of the `.model` card instance `n` (master word
    /// `master`) uses, read in this file or in the kit file that declares
    /// it (as [`Rd::model_view`] reads a card); `None` as
    /// [`Rd::instance_model`].
    pub(in crate::syntax::render) fn instance_params(
        &self,
        n: u32,
        master: &str,
    ) -> Option<Vec<(String, String)>> {
        match self.instance_model(n, master)? {
            (None, card) => Some(self.model_view(card, NONE).params),
            // ponytail: the kit file is parsed again per call (a handful of
            // lines a deck ask); share `spice/xyce_cards`' per-file table
            // when a deck asks for thousands.
            (Some(file), card) => file_models(self.kit()?, file, |_, m| m.params)
                .into_iter()
                .find(|(c, _)| *c == card)
                .map(|(_, params)| params),
        }
    }
}

impl Model {
    /// The card's `level` as it is written (the last one given), 1 when
    /// it gives none or no number: the source dialect's number for the
    /// model, the neutral one through `Behaviours::neutral_level`.
    pub(in crate::syntax::render) fn level(&self) -> u32 {
        self.params
            .iter()
            .rev()
            .find(|(k, _)| k.eq_ignore_ascii_case("level"))
            .and_then(|(_, v)| crate::syntax::render::models::fold(v))
            .map_or(1, |l| l as u32)
    }
}

/// `f` of every `.model` card the kit declares in `file`, by card node: the
/// file's text parsed again (a SPICE-family file; its cards read as
/// [`Rd::model_view`] reads them).
pub(in crate::syntax::render) fn file_models<R>(
    kit: &Kit,
    file: FileId,
    mut f: impl FnMut(u32, Model) -> R,
) -> Vec<(u32, R)> {
    let lex = kit.lang().lexicon().expect("a kit's dialect has a reader");
    let text = &kit.file(file).text;
    let t = tokenize(text, lex);
    let ast = parse(&t, lex);
    assert_eq!(ast.len(), kit.node_count(file), "the kit file's tree");
    let d = kit.lang().behaviours().text_family;
    let mut rd = Rd::new(&ast, &t, d, Some((kit, file)));
    // The file's `.model` names, as `read` sets them: an `ako:` card
    // finds its base (`Rd::derived`).
    if !rd.named() {
        rd.models = rd.model_names(ast.fields(ast.root()));
    }
    let mut out = Vec::new();
    for def in kit.definitions() {
        if def.file == file && matches!(def.kind, DefKind::Model { .. }) && !rd.named() {
            let n = def.node.0;
            out.push((n, f(n, rd.model_view(n, NONE))));
        }
    }
    out
}
