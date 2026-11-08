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
//! - `Rd::kept_construct(&self, tok: u32) -> Construct`
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
    /// is to a writer: one only its own simulator has (Xyce's multi-time,
    /// model-order-reduction and uncertainty analyses, RG analysis
    /// commands `.MPDE`, `.MOR`, `.PCE`, `.EMBEDDEDSAMPLING`, `.RESULT`;
    /// its netlist rewrites `.PREPROCESS`; its own `Y` devices, RG `Y`
    /// devices: neuron, synapse, reduced-order model, reaction network,
    /// general external, accelerometer, power grid, PDE (TCAD), none of
    /// which ngspice, HSPICE, LTspice, Spectre or VACASK has); Xyce's
    /// `.SENS` (an objective's sensitivity to listed parameters over the
    /// deck's `.dc`, `.tran` or `.ac`, RG `.SENS`); HSPICE's StatEye (its
    /// own) and its RF, transient-noise and loop-stability analyses; else
    /// foreign text.
    pub(in crate::syntax::render) fn kept_construct(&self, tok: u32) -> Construct {
        const XY_CARDS: [&str; 6] = [
            ".mpde",
            ".mor",
            ".pce",
            ".embeddedsampling",
            ".result",
            ".preprocess",
        ];
        const XY_DEVICES: [&str; 14] = [
            "neuron",
            "synapse",
            "rom",
            "rxn",
            "genext",
            "acc",
            "pde",
            "pgbr",
            "pgbs",
            "pggb",
            "pgtr",
            "powergridbranch",
            "powergridbusshunt",
            "powergridtransformer",
        ];
        if self.text_behaviours(tok).named_rules == NamedRules::Vacask {
            return match (self.vacask_script(tok), self.t.text(tok)) {
                (true, _) => Construct::VacaskScript,
                (false, "elaborate") => Construct::VacaskElaborate,
                (false, _) => Construct::ForeignText,
            };
        }
        let w = self.t.text(tok).to_ascii_lowercase();
        // One arm per dialect family: a dialect's own words fill its arm.
        match self.text_behaviours(tok).card_rules {
            CardRules::Hspice => match w.as_str() {
                // HSPICE's StatEye, digital vector files, timing checks and
                // power report (HSPICE UG `.STATEYE`, `.VEC`, `.CHECK`,
                // `.POWER`): no other simulator has them.
                ".stateye" | ".vec" | ".check" | ".power" => Construct::PrivateText,
                // HSPICE RF shooting-Newton (`.SN`, `.SNAC`, `.SNNOISE`,
                // `.SNXF`), periodic time-dependent noise (`.PTDNOISE`),
                // sampling noise (`.SAMPLE`), transient noise, loop
                // stability (HSPICE UG): no SPICE target has these analyses.
                ".sn" | ".snac" | ".snnoise" | ".snxf" | ".ptdnoise" | ".sample" | ".trannoise"
                | ".lstb" => Construct::HspiceAnalysis,
                // HSPICE UG, W element: `Wxxx in… ref out… ref N=n
                // RLGCmodel=…` (or `RLGCfile`, `TABLEmodel`, `Umodel`,
                // `FSmodel`, `Smodel`), a lossy multi-conductor line.
                w if w.starts_with('w') && self.w_line(tok) => Construct::HspiceWLine,
                _ => Construct::ForeignText,
            },
            CardRules::Xyce => {
                let private = match w.strip_prefix('y') {
                    Some(ty) => XY_DEVICES.contains(&ty),
                    None => XY_CARDS.contains(&w.as_str()),
                };
                match w.as_str() {
                    _ if private => Construct::PrivateText,
                    ".sens" => Construct::XyceSens,
                    _ => Construct::ForeignText,
                }
            }
            // No LTspice text is its own construct yet: LTspice's private
            // words get their construct in this arm.
            CardRules::Ltspice => Construct::ForeignText,
            CardRules::Spice | CardRules::Ngspice => Construct::ForeignText,
        }
    }
}

// ---- private ----

impl Rd<'_> {
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
