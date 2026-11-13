//! The reader of the circuit owner's IR-text forms (`circuit_text.rs` documents them): one
//! `element` or `model` statement's words in, builder pushes out (`parse_element`,
//! `parse_model`). Total: an unknown key or word is a `TEXT_SYNTAX` row and is skipped; a
//! statement the builder would assert on (a terminal count, a detail row or field outside its
//! domain, a model's word, module or polarity) is one `TEXT_SYNTAX` row and pushes no item, as
//! the kernel rows' reader does, and so is a second element of one whole name in a scope
//! (`Builder::element_named`); the row is pushed through the low tier (`Builder::push`), so what
//! only `check` decides (a class rule) is built and reported. Values are read, names interned and rows pushed in text order
//! (side and wave rows at their field, the detail row after its fields), which is the order the
//! builder calls of the same design make.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9, §7.2 (A4: references are bound by `link` after the text);
//! `text.rs` (`Words`).
//!
//! Contents (crate-private): `pub(crate) fn parse_element`, `pub(crate) fn parse_model`,
//! `pub(crate) fn parse_library`

use crate::build::Builder;
use crate::circuit_check::{Classes, model_ok, verdict};
use crate::circuit_fields::{Field, Fields, Side};
use crate::circuit_text::{FAMILY_WORDS, KIND_WORDS, WAVE_WORDS};
use crate::controlled::{Behav, Controlled, Delay, FreqTable, Laplace, Poly, TableSource};
use crate::design::DetailRow;
use crate::diag::Diags;
use crate::element::{DetailKind, ElemKind, Element, NO_ROW};
use crate::id::{ExprId, ItemRef, List, NameId, Ref, ScopeId};
use crate::item::DefKind;
use crate::library::DeviceLibrary;
use crate::line::Line;
use crate::model::{Model, ModelFamily, Polarity};
use crate::passive::{Coupling, Parasitics, Passive};
use crate::semi::{Semi, Switch};
use crate::source::{Distof, PortSpec, Source};
use crate::text::Words;
use crate::wave::{Am, Exp, Pattern, Pulse, Pwl, Sffm, Sin, TrNoise, TrRandom, WaveKind, WaveRef};

#[derive(Clone, Copy)]
struct NoDetail;

/// The parse context: one statement's cursor and the builder.
struct Cx<'c, 't, 'b> {
    words: &'c mut Words<'t>,
    scope: ScopeId,
    b: &'c mut Builder<'b>,
    diags: &'c mut Diags,
}

/// Reads an element statement after its keyword and pushes it into `scope`. Returns
/// `ItemRef::NONE`, having pushed nothing, when the kind word or the name is missing.
pub(crate) fn parse_element(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let span = words.span();
    let Some(kind) = words
        .next()
        .and_then(|w| KIND_WORDS.iter().position(|&k| k == w))
        .map(|i| ElemKind::ALL[i])
    else {
        words.error(diags);
        return ItemRef::NONE;
    };
    let name = words.name(b, diags);
    if name == NameId::NONE {
        return ItemRef::NONE;
    }
    let terminals = words.names(b, diags);
    let mut head = Element {
        kind,
        name,
        terminals,
        model: Ref::unresolved(NameId::NONE),
        params: List::EMPTY,
        mult: ExprId::NONE,
        detail: NO_ROW,
        mult_law: crate::element::MultLaw::Parallel,
    };
    let mut cx = Cx {
        words: &mut *words,
        scope,
        b: &mut *b,
        diags: &mut *diags,
    };
    head.detail = match kind.detail() {
        DetailKind::None => {
            cx.read_fields(&mut NoDetail, Some(&mut head), false);
            NO_ROW
        }
        DetailKind::Passive => cx.read_detail::<Passive>(&mut head),
        DetailKind::Coupling => cx.read_detail::<Coupling>(&mut head),
        DetailKind::Source => cx.read_detail::<Source>(&mut head),
        DetailKind::Controlled => cx.read_detail::<Controlled>(&mut head),
        DetailKind::Poly => cx.read_detail::<Poly>(&mut head),
        DetailKind::Table => cx.read_detail::<TableSource>(&mut head),
        DetailKind::Behav => cx.read_detail::<Behav>(&mut head),
        DetailKind::Laplace => cx.read_detail::<Laplace>(&mut head),
        DetailKind::FreqTable => cx.read_detail::<FreqTable>(&mut head),
        DetailKind::Delay => cx.read_detail::<Delay>(&mut head),
        DetailKind::Semi => cx.read_detail::<Semi>(&mut head),
        DetailKind::Switch => cx.read_detail::<Switch>(&mut head),
        DetailKind::Line => cx.read_detail::<Line>(&mut head),
    };
    if kind == ElemKind::Unresolved {
        // never linked (`master_kind` is `None`), so `name?`'s held mark is cleared here
        head.model.def = crate::id::DefId::NONE;
    }
    // what `Builder::element` asserts on, and a second element of the name in `scope` (its
    // detail and side rows stay unreferenced)
    let (lo, hi) = kind.terminals();
    let count = head.terminals.len();
    let v = verdict(b.design(), &head, Classes::Assume);
    if count < lo as usize
        || count > hi as usize
        || !v.rows
        || !v.domain
        || b.element_named(scope, name).is_some()
    {
        words.error(diags);
        return ItemRef::NONE;
    }
    b.push(scope, head, span)
}

/// Reads a model statement after its keyword, pushes it into `scope` and defines it. Returns
/// `ItemRef::NONE`, having pushed nothing, when the name or family is missing or the scope
/// already defines a model of that name.
pub(crate) fn parse_model(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let span = words.span();
    let name = words.name(b, diags);
    if name == NameId::NONE {
        return ItemRef::NONE;
    }
    let Some(family) = words
        .next()
        .and_then(|w| FAMILY_WORDS.iter().position(|&k| k == w))
        .map(|i| ModelFamily::ALL[i])
    else {
        words.error(diags);
        return ItemRef::NONE;
    };
    let mut m = Model {
        name,
        params: List::EMPTY,
        bin_of: NameId::NONE,
        ako: Ref::unresolved(NameId::NONE),
        module: Ref::unresolved(NameId::NONE),
        version: ExprId::NONE,
        word: NameId::NONE,
        family,
        polarity: Polarity::None,
    };
    // bounded: each pass consumes at least one word of the line
    while !words.at_end() {
        let Some(key) = words.key() else {
            words.error(diags);
            words.next();
            continue;
        };
        match key {
            "polarity" => match words.next() {
                Some("n") => m.polarity = Polarity::N,
                Some("p") => m.polarity = Polarity::P,
                _ => words.error(diags),
            },
            "params" => m.params = words.params(scope, b, diags),
            "bin_of" => m.bin_of = words.name(b, diags),
            "ako" => m.ako = words.reference(b, diags),
            "module" => m.module = words.reference(b, diags),
            "version" => m.version = words.value(scope, b, diags),
            "word" => m.word = words.name(b, diags),
            _ => {
                words.error(diags);
                words.next();
            }
        }
    }
    // what `Builder::model` asserts on, and a second model of the name
    let code = matches!(family, ModelFamily::CodeModel | ModelFamily::Other);
    if (m.word != NameId::NONE) != code
        || (m.module.name != NameId::NONE) != (family == ModelFamily::Module)
        || !model_ok(b.design(), &m)
        || b.design()
            .def_index
            .contains_key(&(scope, DefKind::Model, name))
    {
        words.error(diags);
        return ItemRef::NONE;
    }
    let item = b.push(scope, m, span);
    b.define_model(scope, item, name, m.bin_of);
    item
}

/// Reads a device library's fields after its keyword and pushes it into `scope`. Returns
/// `ItemRef::NONE`, having pushed nothing, when the family word is missing.
pub(crate) fn parse_library(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> ItemRef {
    let span = words.span();
    let Some(family) = words
        .next()
        .and_then(|w| FAMILY_WORDS.iter().position(|&k| k == w))
        .map(|i| ModelFamily::ALL[i])
    else {
        words.error(diags);
        return ItemRef::NONE;
    };
    let mut path = NameId::NONE;
    // bounded: each pass consumes at least one word of the line
    while !words.at_end() {
        match words.key() {
            Some("path") => match words.path(diags) {
                Some(p) => path = b.name(&p),
                None => return ItemRef::NONE,
            },
            _ => {
                words.error(diags);
                words.next();
            }
        }
    }
    b.device_library(scope, DeviceLibrary { path, family }, span)
}

impl Fields for NoDetail {
    const BLANK: Self = NoDetail;
    fn fields(&mut self, _: &mut dyn FnMut(&'static str, Field<'_>)) {}
}

// ---- private ----

impl Cx<'_, '_, '_> {
    /// Reads the header fields and `D`'s fields, then pushes the `D` row.
    fn read_detail<D: Fields + DetailRow>(&mut self, head: &mut Element) -> u32 {
        let mut row = D::BLANK;
        self.read_fields(&mut row, Some(head), false);
        self.b.detail(row)
    }

    /// Reads `(fields)` of a side or wave row after its `(`, then pushes it.
    fn read_nested<D: Fields + DetailRow>(&mut self) -> u32 {
        let mut row = D::BLANK;
        self.read_fields(&mut row, None, true);
        self.b.detail(row)
    }

    /// Reads `key=value` fields to the end of the line (`close`: to the matching `)`), into
    /// the header's fields when `head` is given and into `row`'s.
    fn read_fields<R: Fields>(&mut self, row: &mut R, mut head: Option<&mut Element>, close: bool) {
        // bounded: each pass consumes at least one word of the line
        loop {
            if close && self.words.peek() == Some(")") {
                self.words.next();
                return;
            }
            if self.words.at_end() {
                if close {
                    self.words.error(self.diags);
                }
                return;
            }
            let Some(key) = self.words.key() else {
                self.words.error(self.diags);
                self.words.next();
                continue;
            };
            if let Some(h) = head.as_deref_mut() {
                match key {
                    "model" => {
                        h.model = self.words.reference(self.b, self.diags);
                        continue;
                    }
                    "params" => {
                        h.params = self.words.params(self.scope, self.b, self.diags);
                        continue;
                    }
                    "mult" => {
                        h.mult = self.words.value(self.scope, self.b, self.diags);
                        continue;
                    }
                    "mult_law" => {
                        let mut law = h.mult_law;
                        self.read_field(Field::Choice(&mut law));
                        h.mult_law = law;
                        continue;
                    }
                    _ => {}
                }
            }
            let mut found = false;
            row.fields(&mut |name, field| {
                if name == key {
                    found = true;
                    self.read_field(field);
                }
            });
            if !found {
                self.words.error(self.diags);
                self.words.next();
            }
        }
    }

    /// Reads one field's value.
    fn read_field(&mut self, field: Field<'_>) {
        match field {
            Field::Expr(e) | Field::Dyn(e) => *e = self.words.value(self.scope, self.b, self.diags),
            Field::Name(n) => *n = self.words.name(self.b, self.diags),
            Field::Names(l) => *l = self.words.names(self.b, self.diags),
            Field::Exprs(l) => *l = self.words.exprs(self.scope, self.b, self.diags),
            Field::Count(c) => match self.words.next().and_then(|w| w.parse::<u8>().ok()) {
                Some(v) => *c = v,
                None => self.words.error(self.diags),
            },
            Field::Flag(f) => match self.words.next() {
                Some("false") => *f = false,
                Some("true") => *f = true,
                _ => self.words.error(self.diags),
            },
            Field::Choice(c) => {
                let words = c.words();
                match self
                    .words
                    .next()
                    .and_then(|w| words.iter().position(|&x| x == w))
                {
                    Some(i) => c.set(i),
                    None => self.words.error(self.diags),
                }
            }
            Field::Side(index, side) => {
                if self.words.expect("(", self.diags) {
                    *index = match side {
                        Side::Parasitics => self.read_nested::<Parasitics>(),
                        Side::Port => self.read_nested::<PortSpec>(),
                        Side::Distof => self.read_nested::<Distof>(),
                    };
                }
            }
            Field::Wave(w) => {
                let kind = self
                    .words
                    .next()
                    .and_then(|x| WAVE_WORDS.iter().position(|&k| k == x))
                    .map(|i| WaveKind::ALL[i]);
                let Some(kind) = kind else {
                    self.words.error(self.diags);
                    return;
                };
                if !self.words.expect("(", self.diags) {
                    return;
                }
                let index = match kind {
                    WaveKind::Pulse => self.read_nested::<Pulse>(),
                    WaveKind::Sin => self.read_nested::<Sin>(),
                    WaveKind::Exp => self.read_nested::<Exp>(),
                    WaveKind::Pwl => self.read_nested::<Pwl>(),
                    WaveKind::Sffm => self.read_nested::<Sffm>(),
                    WaveKind::Am => self.read_nested::<Am>(),
                    WaveKind::TrNoise => self.read_nested::<TrNoise>(),
                    WaveKind::TrRandom => self.read_nested::<TrRandom>(),
                    WaveKind::Pattern => self.read_nested::<Pattern>(),
                };
                *w = WaveRef { kind, index };
            }
        }
    }
}
