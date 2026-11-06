//! The Python API's decks, declared with the definition API: ngspice text
//! written from what a caller gave (`python::deck`), not read from a file.
//! Its numbers are read back correctly rounded, so a double's SI spelling
//! (`unit::format_spice_number`) reads back as that double, where ngspice's
//! digit-by-digit arithmetic can land an ulp away (`100u` is
//! 9.999999999999999e-5 there); it holds a `.step` (Xyce's form, `.step
//! [lin|dec|oct] name start stop step`), which ngspice lacks and each
//! target writes in its own form or refuses; and its `simulator
//! lang=<dialect>` regions hold what only that dialect says (a Spectre
//! `sweep`, a line given as one dialect's text; `lang=ngspice` is
//! ngspice's), read by that dialect's lexicon until `simulator lang=spice`
//! and kept as that dialect's text. A reader only: written, it is ngspice.
//!
//! Cites: none (ngspice's own definition cites its manual).
//!
//! Contents:
//! - `pub(super) fn ngspice_given() -> DialectDef`

use crate::syntax::Lang;
use crate::syntax::def::DialectDef;
use crate::syntax::lexicon::Arithmetic;
use crate::syntax::token::Tag;

/// ngspice's text with decimal numbers, `.step` and dialect regions.
pub(super) fn ngspice_given() -> DialectDef {
    DialectDef::new("ngspice-given")
        .extends(Lang::NGSPICE)
        .text_family(Lang::NGSPICE)
        .arithmetic(Arithmetic::Decimal)
        .keyword(".step", Tag::KwStep)
        .region("ngspice", Lang::NGSPICE)
        .region("spectre", Lang::SPECTRE)
        .region("vacask", Lang::VACASK)
        .region("hspice", Lang::HSPICE)
        .region("ltspice", Lang::LTSPICE)
        .region("xyce", Lang::XYCE)
}
