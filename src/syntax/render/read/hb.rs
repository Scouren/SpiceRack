//! A `.hb` card of one tone → its [`Card::Hb`]: the tone and the harmonic
//! count, by the card's own form. Xyce's `.HB f` takes its count from the
//! deck's first `.OPTIONS HBINT NUMFREQ=` (10 with none); HSPICE's card is
//! `.HB TONES=f NHARMS=k`, and ESPice's HSPICE decks write `.HB f k`. A
//! card of two tones, a count that is no whole number, and a Xyce
//! oscillator analysis (`HBOSC`: the tone is a first guess and the circuit
//! drives itself) are not read: each stays the card's kept text.
//!
//! Cites: Xyce RG 7.10 §2.1.13 (`.HB`), §2.1.25.17 Table 2-14 (`NUMFREQ`,
//! default 10); Xyce 7.10 runs ("duplicate HBINT parameter. Using the
//! first value found"; `hbosc=1`: a 0.584 V oscillation with no source);
//! HSPICE Reference Manual: Commands and Control Options E-2010.12 p.115
//! (`.HB TONES= NHARMS=`); ESPice `tests/netlist/hbnoise/lti_rc_hspice.sp`
//! (`.hb 1e5 5`); ZERO-LEDGER P5-004 step 2, P5-007, P5-079.
//!
//! Contents:
//! - `Rd::hb_card(&self, n: u32) -> Option<Card>`

use super::Rd;
use super::options::Opt;
use super::testbench::Card;
use crate::syntax::ast::{Node, Tag};
use crate::syntax::behaviour::{CardRules, DotRule};

/// The harmonics Xyce computes when the deck sets no `NUMFREQ` (RG 7.10
/// Table 2-14; 21 rows of its two-sided spectrum, run).
const XYCE_NUMFREQ: u16 = 10;

impl Rd<'_> {
    /// The `.hb` card `n` (its `DotRule::Hb` row sent it) when it is one
    /// driven tone: the tone in hertz and the harmonics above DC. `None`
    /// for any other form (two tones, an oscillator, a count that is
    /// none), which stays the card's kept text.
    pub(super) fn hb_card(&self, n: u32) -> Option<Card> {
        let rule = self.dot_rule(&self.head_kind(n));
        assert!(rule == Some(DotRule::Hb), "a card its `Hb` rule sent");
        let count = |k: u32| self.count(k).and_then(|h| u16::try_from(h).ok());
        let rhs = |a: u32| self.ast.rhs(Node(a));
        let (tone, harms) = match (self.b.card_rules, self.fields(n)) {
            (CardRules::Xyce, &[f]) if self.numw(f) => {
                // An oscillator's tone is a first guess, not a drive.
                if self.hbint("hbosc").is_some_and(|v| v != "0") {
                    return None;
                }
                let harms = match self.hbint("numfreq") {
                    Some(v) => v.parse().ok()?,
                    None => XYCE_NUMFREQ,
                };
                (self.number(f)?, harms)
            }
            (CardRules::Hspice, &[f, k]) if self.numw(f) && self.numw(k) => {
                (self.number(f)?, count(k)?)
            }
            (CardRules::Hspice, &[f, k]) if self.keyed(f, "tones") && self.keyed(k, "nharms") => {
                (self.number(rhs(f))?, count(rhs(k))?)
            }
            _ => return None,
        };
        let read = tone > 0.0 && tone.is_finite() && harms > 0;
        read.then_some(Card::Hb(tone, harms))
    }
}

// ---- private ----

impl Rd<'_> {
    /// The value, as written, of the deck's first `.OPTIONS HBINT <key>=`
    /// (Xyce 7.10 takes the first of two: run).
    fn hbint(&self, key: &str) -> Option<String> {
        let key = format!("hbint.{key}");
        // ponytail: the top file's cards only, as `Rd::top_ac` reads the
        // `.ac`; an `hbint` card in an included file is not seen (the
        // count is then Xyce's default, an `hbosc` there is missed). Walk
        // the includes when a deck keeps its options in one.
        let top = self.ast.fields(self.ast.root()).iter();
        top.filter(|&&s| self.tag(s) == Tag::Options && self.options_ok(s))
            .flat_map(|&s| self.options_view(s))
            .find_map(|o| match o {
                Opt::Xyce(k, v) if k == key => Some(v),
                _ => None,
            })
    }

    /// Whether item `a` is `key=value`, the key in any case.
    fn keyed(&self, a: u32, key: &str) -> bool {
        self.tag(a) == Tag::Assign && self.key(a).eq_ignore_ascii_case(key)
    }
}
