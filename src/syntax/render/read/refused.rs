//! A card the old grammar refused → `Raw` or `Unparsed`, as
//! the old SPICE grammar sorted them: a dot card whose word is a `rawkind`
//! (or an overlay's own raw word) was `Raw`, any other `Unparsed`.
//!
//! Cites: the old SPICE grammar's `Raw`, `rawkind`, `Unparsed`; the old LTspice grammar's `Raw_lt`;
//! the old Xyce grammar's `Raw_xy`.
//!
//! Contents:
//! - `Rd::refused(&self, n: u32) -> Kept`
//! - `Rd::dialect_raw(&self, w: &str) -> bool`
//! - `Rd::kept_construct(&self, tok: u32) -> Construct` (by `Behaviours::kept_words`)
//! - `Rd::global_swept(&self, n: u32) -> bool`

use super::{Kept, Rd};
use crate::syntax::ast::{Node, Tag};
use crate::syntax::behaviour::{CardRules, NamedRules};
use crate::syntax::render::capability::Construct;
use crate::syntax::token::Tag as T;

/// the old SPICE grammar's `rawkind`: the dot words whose card, when no rule reads it,
/// is `Raw`; any other dot card no rule reads is `Unparsed`.
const RAWKIND: &[&str] = &[
    "acphasenoise",
    "osdi_include",
    "appendmodel",
    "temperature",
    "montecarlo",
    "phasenoise",
    "tran_noise",
    "trannoise",
    "unprotect",
    "envelope",
    "ptdnoise",
    "pre_osdi",
    "acmatch",
    "dcmatch",
    "hbnoise",
    "snnoise",
    "measure",
    "biaschk",
    "stimuli",
    "connect",
    "protect",
    "options",
    "nodeset",
    "include",
    "verilog",
    "dcsens",
    "pnoise",
    "jitter",
    "sample",
    "unprot",
    "option",
    "dcvolt",
    "global",
    "dcinc",
    "disto",
    "envlp",
    "hbosc",
    "hblin",
    "matex",
    "noise",
    "snosc",
    "check",
    "power",
    "store",
    "mosra",
    "print",
    "probe",
    "graph",
    "width",
    "title",
    "model",
    "acxf",
    "dcxf",
    "four",
    "hbac",
    "hbxf",
    "lstb",
    "qpss",
    "sens",
    "snac",
    "snxf",
    "tran",
    "meas",
    "dout",
    "stim",
    "plot",
    "prot",
    "step",
    "endc",
    "opts",
    "temp",
    "save",
    "fft",
    "lin",
    "net",
    "pac",
    "pss",
    "pxf",
    "stb",
    "pat",
    "opt",
    "hdl",
    "ac",
    "dc",
    "hb",
    "mc",
    "op",
    "pz",
    "sn",
    "sp",
    "tf",
    "ic",
];

impl<'a> Rd<'a> {
    /// quirk: the old SPICE grammar's `Raw <- '.' rawkind !namechar rest /
    /// ends rest`: a dot card no rule reads is `Raw` when its word is a `rawkind` (or
    /// `.ends`/`.eom`), else `Unparsed` (and so are non-dot lines).
    pub(in crate::syntax::render) fn refused(&self, n: u32) -> Kept {
        // The dot word as the old SPICE grammar's `!namechar` reads it: the head token
        // and the name bytes touching it (`.probe/csd` is no `.probe`).
        let first = self.main(n);
        let last = self.line_last(first);
        let mut end = first;
        while end < last
            && self.t.end(end) == self.t.start(end + 1)
            && !matches!(
                self.ttag(end + 1),
                T::Comma
                    | T::Equal
                    | T::EqualEqual
                    | T::BangEqual
                    | T::LessEqual
                    | T::GreaterEqual
                    | T::LParen
                    | T::RParen
                    | T::Quote
                    | T::DoubleQuote
                    | T::String
                    | T::LBrace
                    | T::RBrace
            )
        {
            end += 1;
        }
        let word = self.span(first, end).to_ascii_lowercase();
        let Some(w) = word.strip_prefix('.') else {
            return Kept::Unparsed;
        };
        if w == "ends" || w == "eom" || self.dialect_raw(w) || RAWKIND.contains(&w) {
            Kept::Raw
        } else {
            Kept::Unparsed
        }
    }

    /// quirk: whether `w` is one of the old dialect overlays' own raw dot
    /// words (the old LTspice grammar's `Raw_lt`, the old Xyce grammar's
    /// `Raw_xy`), listed below: such a card the grammar read and kept raw.
    pub(in crate::syntax::render) fn dialect_raw(&self, w: &str) -> bool {
        const LT: &[&str] = &[
            "endmachine",
            "savestate",
            "loadstate",
            "softstart",
            "savebias",
            "loadbias",
            "machine",
            "fourier",
            "output",
            "ferret",
            "state",
            "wave",
            "rule",
            "endl",
            "fra",
        ];
        const XY: &[&str] = &[
            "embeddedsampling",
            "temperature",
            "preprocess",
            "objective",
            "sampling",
            "initcond",
            "result",
            "mpde",
            "dcop",
            "temp",
            "save",
            "sens",
            "pce",
            "mor",
            "rol",
        ];
        match self.b.card_rules {
            CardRules::Ltspice => LT.contains(&w),
            CardRules::Xyce => XY.contains(&w),
            CardRules::Spice | CardRules::Ngspice | CardRules::Hspice => false,
        }
    }
}

impl Rd<'_> {
    /// Whether the Xyce `.global_param` card `n` assigns a value that
    /// reads, itself or through another `.global_param`, a variable Xyce
    /// sweeps (`temp`, `vt`, `freq`, `hertz`, `time`): Xyce re-evaluates
    /// such a value as the run goes, a `.param` it fixes once (Xyce 7.10
    /// runs `{TEMP+10}` and `{log10(freq)+1}` otherwise as `.param`). Such a
    /// card is kept as text; any other is a `.param`.
    pub(in crate::syntax::render) fn global_swept(&self, n: u32) -> bool {
        const SWEPT: [&str; 5] = ["temp", "vt", "freq", "hertz", "time"];
        let assigns = |c: u32| {
            self.fields(c)
                .iter()
                .copied()
                .filter(move |&a| self.tag(a) == Tag::Assign)
        };
        let cards: Vec<u32> = self
            .ast
            .fields(self.ast.root())
            .iter()
            .copied()
            .filter(|&c| self.tag(c) == Tag::Param && self.head_kind(c) == "global_param")
            .collect();
        // ponytail: the deck's `.global_param` cards scanned per card; a
        // set built once in `read` if a deck holds thousands.
        let mut swept: Vec<String> = Vec::new();
        // Each pass adds a name or ends, so it runs at most one pass per card.
        for _ in 0..=cards.len() {
            let before = swept.len();
            for a in cards.iter().flat_map(|&c| assigns(c)) {
                let name = self.text(self.ast.lhs(Node(a))).to_ascii_lowercase();
                let body = self.vtext(self.ast.rhs(Node(a))).to_ascii_lowercase();
                let mut words = body.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'));
                if !swept.contains(&name)
                    && words.any(|w| SWEPT.contains(&w) || swept.iter().any(|s| s == w))
                {
                    swept.push(name);
                }
            }
            if swept.len() == before {
                break;
            }
        }
        assigns(n).any(|a| swept.contains(&self.text(self.ast.lhs(Node(a))).to_ascii_lowercase()))
    }

    /// What the statement kept as text from another dialect at token `tok`
    /// is to a writer: a VACASK script line or `elaborate`, else the
    /// construct its language's `kept_word` rows give its first word
    /// (`Behaviours::kept_words`: HSPICE's own analyses and W line, Xyce's
    /// own analyses and `Y` devices, …), else foreign text.
    pub(in crate::syntax::render) fn kept_construct(&self, tok: u32) -> Construct {
        if self.text_behaviours(tok).named_rules == NamedRules::Vacask {
            return match (self.vacask_script(tok), self.t.text(tok)) {
                (true, _) => Construct::VacaskScript,
                (false, "elaborate") => Construct::VacaskElaborate,
                (false, _) => Construct::ForeignText,
            };
        }
        let w = self.t.text(tok).to_ascii_lowercase();
        let words = self.text_behaviours(tok).kept_words;
        // A whole word's row, else its letter's (an element line), whose
        // construct adds its own shape check.
        let whole = words.iter().find(|(k, _)| k.len() > 1 && *k == w);
        let letter = words
            .iter()
            .find(|(k, _)| k.len() == 1 && w.starts_with(*k));
        match (whole, letter) {
            (Some(&(_, c)), _) => c,
            (None, Some(&(_, c))) if self.kept_shape(c, tok) => c,
            (None, _) => Construct::ForeignText,
        }
    }
}

// ---- private ----

impl Rd<'_> {
    /// Whether the element line at `tok` has the shape construct `c` needs
    /// when a one-letter `kept_word` row matched it (rung 4: one arm per
    /// construct that checks one).
    fn kept_shape(&self, c: Construct, tok: u32) -> bool {
        match c {
            Construct::HspiceWLine => self.w_line(tok),
            Construct::PrivateText => self.private_line(tok),
            // else: a construct with no shape of its own takes every line.
            _ => true,
        }
    }

    /// Whether the element line at `tok`, matched by a one-letter
    /// `PrivateText` row, is its simulator's own device. A line of any
    /// letter is, except `K`: every simulator couples inductors, so a `K`
    /// line is private only through a core model, a model name after the
    /// coupling value (Xyce RG 7.10 §2.3.6: `K<name> L<inductor name>
    /// [L<inductor name>*] <coupling value> [model name]`).
    fn private_line(&self, tok: u32) -> bool {
        let line = self.span(tok, self.line_last(tok));
        assert!(!line.is_empty(), "a kept line holds its first word");
        if !line.starts_with(['k', 'K']) {
            return true;
        }
        let mut words = line.split_whitespace().rev();
        let (Some(model), Some(k)) = (words.next(), words.next()) else {
            return false;
        };
        // The coupling value: a number, or an expression's closing
        // delimiter.
        let value = k.ends_with(['}', '\''])
            || k.trim_start_matches(['+', '-'])
                .starts_with(|c: char| c.is_ascii_digit() || c == '.');
        let name = model.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_')
            && model
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_-.".contains(c));
        value && name
    }

    /// Whether the HSPICE text at `tok` is a W element's line: one of its
    /// model keys on the card (HSPICE UG, W element).
    fn w_line(&self, tok: u32) -> bool {
        const KEYS: [&str; 6] = [
            "rlgcmodel",
            "rlgcfile",
            "tablemodel",
            "umodel",
            "fsmodel",
            "smodel",
        ];
        let line = self.span(tok, self.line_last(tok)).to_ascii_lowercase();
        line.split(|c: char| c.is_whitespace()).any(|w| {
            KEYS.iter()
                .any(|k| w.strip_prefix(k).is_some_and(|r| r.starts_with('=')))
        })
    }
}
