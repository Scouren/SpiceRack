//! Lines a source's own simulator rejects before it runs the deck: such a
//! line has no meaning to write, so the reader leaves it out with a note
//! naming the cause (`P0010` Xyce, `P0011` an ngspice PWL `r=`). Only the
//! classes visible from the line itself are here; a deck its simulator
//! rejects for another file or for what two lines mean together is named
//! in `tests/fixtures/conformance_noise.txt`.
//!
//! Cites: Xyce 7.10 `-norun` on each class (the messages in `P0010`'s
//! cite); Xyce Reference Guide 7.10 §2.1.8 (`.FFT`: the window names,
//! FORMAT, NP); ngspice-45 runs (`P0011`'s cite, ngspice manual §4.1.4).
//!
//! Contents:
//! - `Rd::rejected(&self, n: u32) -> Option<Code>`

use super::Rd;
use crate::syntax::ast::Tag;
use crate::syntax::behaviour::CardRules;
use crate::syntax::diag::Code;
use crate::syntax::token::Tag as T;

/// A PWL's literal values after its keyword, as the line gives them.
struct Pwl {
    /// The time/value numbers, flat.
    points: Vec<f64>,
    /// `r=` as a number.
    repeat: Option<f64>,
    /// A value that is no plain number (an expression, a parameter): the
    /// points' times are not known from the line.
    expr: bool,
    /// `file`: the points are in another file.
    file: bool,
}

impl Rd<'_> {
    /// The note for statement `n` when its own simulator rejects it before
    /// running: `P0010` for a Xyce line, `P0011` for an ngspice PWL `r=`
    /// no time point of the wave; `None` for any other line.
    pub(in crate::syntax::render) fn rejected(&self, n: u32) -> Option<Code> {
        // A block's head is no line of these classes.
        if matches!(
            self.tag(n),
            Tag::Subckt
                | Tag::Section
                | Tag::Library
                | Tag::If
                | Tag::Else
                | Tag::Control
                | Tag::Region
                | Tag::Data
                | Tag::Title
                | Tag::End
        ) {
            return None;
        }
        let first = self.main(n);
        let last = self.line_last(first);
        assert!(first <= last, "a statement has its head");
        match self.text_behaviours(first).card_rules {
            CardRules::Xyce => self.xyce_rejects(first, last).then_some(Code::P0010),
            CardRules::Ngspice => self.ngspice_rejects(first, last).then_some(Code::P0011),
            CardRules::Spice | CardRules::Hspice | CardRules::Ltspice => None,
        }
    }
}

// ---- private ----

impl Pwl {
    /// The time of each whole time/value pair (a dangling last value is
    /// no point).
    fn times(&self) -> Vec<f64> {
        self.points.chunks_exact(2).map(|c| c[0]).collect()
    }
}

impl Rd<'_> {
    /// Xyce 7.10 `-norun` rejects the line `first..=last`: a card's
    /// fields (an analysis's count, `.FFT`'s and `.MEASURE`'s words, a
    /// `.MEASURE`'s delimiter with no partner or key with no value,
    /// `.SENS`'s vectors), an element's (a letter Xyce has no device for,
    /// too few nodes, values past the device's, a PWL or PAT wave), or a
    /// spelling no Xyce line reads (an unclosed brace, `{POLY` with no
    /// count, an exponent in braces, a lead current of two names).
    fn xyce_rejects(&self, first: u32, last: u32) -> bool {
        let head = self.t.text(first).to_ascii_lowercase();
        let p = self.pieces(first, last);
        if self.xyce_spelling(first, last) {
            return true;
        }
        if head.starts_with('.') {
            return self.xyce_card(&head, &p);
        }
        // `L=1E-4 …`: a continuation's keys cut off by a comment line
        // (Xyce_Regression BUG_250, which Xyce runs), no element.
        if first < last && self.ttag(first + 1) == T::Equal {
            return false;
        }
        let pos = self.positional(&p);
        let bare = |q: &(u32, u32)| q.0 == q.1 && self.ttag(q.0) == T::Number;
        // A `{` or `'` that opens a field (after a blank, or touching the
        // name) and never closes: Xyce reads the rest as one value ("Not
        // enough fields", "Invalid DC value"); one inside a field (`1{`)
        // is a name character.
        let open = (first + 1..=last).any(|k| {
            let opens = k == first + 1 || self.t.end(k - 1) != self.t.start(k);
            let closer = match self.ttag(k) {
                T::LBrace => T::RBrace,
                T::Quote => T::Quote,
                _ => return false,
            };
            opens && !(k + 1..=last).any(|j| self.ttag(j) == closer)
        });
        open || match head.as_bytes()[0].to_ascii_uppercase() {
            // "Unrecognized line"; "Invalid device type".
            b'#' | b'A' | b'N' => true,
            b'R' | b'L' | b'C' => {
                let rest = pos.get(2..).unwrap_or_default();
                pos.len() < 2 || rest.len() > 2 || (rest.len() == 2 && rest.iter().all(bare))
            }
            // "Model is required for device".
            b'D' => self.words(first, last) < 4,
            // A second coupling value: "Specified model not found".
            b'K' => pos.iter().filter(|&q| bare(q)).count() > 1,
            // Four nodes, then keys only: "Unrecognized parameter".
            b'T' => pos.len() > 4,
            b'V' | b'I' => {
                let pwl = self.pwl(first, last).is_some_and(|w| {
                    let times = w.times();
                    let no_points = w.points.is_empty() && !w.expr && !w.file;
                    let off = match (w.repeat, times.last(), w.expr) {
                        (Some(r), Some(&end), false) => r < 0.0 || r >= end,
                        _ => false,
                    };
                    no_points || off
                });
                // "Not enough fields on input line": the name and two nodes.
                self.words(first, last) < 3 || pwl || self.pat_rejected(first, last)
            }
            _ => false,
        }
    }

    /// Xyce's dot card `head` with fields `p` that `-norun` rejects.
    fn xyce_card(&self, head: &str, p: &[(u32, u32)]) -> bool {
        const MODES: [&str; 10] = [
            "tran",
            "tr",
            "dc",
            "ac",
            "fft",
            "noise",
            "tran_cont",
            "dc_cont",
            "ac_cont",
            "noise_cont",
        ];
        // Xyce RG `.FFT`; `.MEASURE` (FFT measures).
        const WINDOWS: [&str; 19] = [
            "rect",
            "rectangular",
            "bart",
            "bartlett",
            "bartletthann",
            "black",
            "blackman",
            "hamm",
            "hamming",
            "hann",
            "hanning",
            "harris",
            "blackmanharris",
            "nuttall",
            "cosine2",
            "cosine4",
            "halfcyclesine",
            "halfcyclesine3",
            "halfcyclesine6",
        ];
        const FFT_TYPES: [&str; 8] = ["enob", "eqn", "param", "find", "sfdr", "sndr", "snr", "thd"];
        const ONE_VAR: [&str; 7] = ["avg", "max", "min", "pp", "rms", "integ", "integral"];
        let word = |i: usize| p.get(i).map(|&(a, b)| self.span(a, b).to_ascii_lowercase());
        let words: Vec<String> = (0..p.len())
            .map(|i| word(i).expect("i < p.len()"))
            .collect();
        assert!(head.starts_with('.'), "a card's head");
        assert_eq!(words.len(), p.len(), "a word per field");
        let key = |k: &str| {
            words
                .iter()
                .find_map(|w| w.strip_prefix(k).and_then(|v| v.strip_prefix('=')))
        };
        let sweep =
            |w: Option<String>| w.is_some_and(|w| matches!(w.as_str(), "dec" | "oct" | "lin"));
        match head {
            ".tran" => p.len() < 2,
            ".four" => p.len() < 2,
            ".ac" => sweep(word(0)) && p.len() != 4,
            // `.DC v start stop step …`: four fields per linear sweep.
            ".dc" => {
                let keyed = words.iter().any(|w| {
                    sweep(Some(w.clone()))
                        || w == "list"
                        || w.contains('=')
                        || w.starts_with("data")
                });
                !keyed && !p.len().is_multiple_of(4)
            }
            // `.NOISE out src type n fstart fstop [pts]`.
            ".noise" => (0..p.len())
                .find(|&i| sweep(word(i)))
                .is_some_and(|i| !matches!(p.len() - i, 4 | 5)),
            // "Invalid WINDOW type", "Invalid FORMAT type", "NP value …
            // must be … >=4" (NP=0; Xyce rounds a positive NP).
            ".fft" => {
                p.is_empty()
                    || key("window").is_some_and(|w| !WINDOWS.contains(&w))
                    || key("format").is_some_and(|f| !matches!(f, "norm" | "unorm"))
                    || key("np")
                        .and_then(|n| n.parse::<f64>().ok())
                        .is_some_and(|n| n < 1.0)
            }
            ".measure" | ".meas" => {
                let (mode, name, kind) = (word(0), word(1), word(2));
                let is_mode =
                    |w: &Option<String>| w.as_ref().is_some_and(|w| MODES.contains(&w.as_str()));
                let has = |w: &str| words.iter().any(|x| x == w);
                let fft = mode.as_deref() == Some("fft");
                let kind = kind.unwrap_or_default();
                // A brace, parenthesis or quote with no partner ("Unmatched
                // expression delimiter", "Invalid PAR() expression syntax",
                // "Invalid () expression syntax", "Could not parse measured
                // variable"), a key the line ends after ("Incomplete
                // .MEASURE line", "Invalid Measure Line").
                let (a, b) = (p.first().map_or(1, |f| f.0), p.last().map_or(0, |l| l.1));
                let count = |t: T| (a..=b).filter(|&k| self.ttag(k) == t).count();
                let unpaired = count(T::LBrace) != count(T::RBrace)
                    || count(T::LParen) != count(T::RParen)
                    || !count(T::Quote).is_multiple_of(2)
                    || (a <= b && self.ttag(b) == T::Equal);
                !is_mode(&mode)
                    || is_mode(&name)
                    || (ONE_VAR.contains(&kind.as_str()) && p.len() == 3)
                    || (kind == "trig" && !has("targ"))
                    || words.last().is_some_and(|w| w == "when")
                    || (fft
                        && !FFT_TYPES
                            .iter()
                            .any(|t| kind == *t || kind.starts_with(&format!("{t}="))))
                    || (fft && kind == "find" && !has("at") && key("at").is_none())
                    || unpaired
            }
            // "Possibly a missing comma in vector parameter" (braces with
            // no comma between, a list ending in `,` before the next key, a
            // value with no comma before it); "Vector parameter begins with
            // a comma", "Error parsing vector parameter" (a comma after
            // `=`, after a comma, or ending the line).
            ".sens" => {
                let comma = |k: u32| self.ttag(k) == T::Comma;
                let end = p.last().map_or(0, |q| q.1);
                let stray = p.first().is_some_and(|f| {
                    (f.0..=end).any(|k| {
                        comma(k) && (k == end || comma(k + 1) || self.ttag(k - 1) == T::Equal)
                    })
                });
                // A value before the first key is read past (`.sens v(a)` runs).
                let key = p
                    .first()
                    .and_then(|f| (f.0..=end).find(|&k| self.ttag(k) == T::Equal));
                let bare = key.is_some_and(|eq| {
                    let pos = self.positional(p);
                    pos.iter().any(|&(a, _)| a > eq && !comma(a))
                });
                let list = |(i, &(a, b)): (usize, &(u32, u32))| {
                    let key_next = p
                        .get(i + 1)
                        .is_some_and(|&(c, d)| (c..=d).any(|k| self.ttag(k) == T::Equal));
                    (comma(b) && key_next)
                        || (a..b).any(|k| {
                            self.ttag(k) == T::RBrace
                                && self.ttag(k + 1) == T::LBrace
                                && self.t.end(k) == self.t.start(k + 1)
                        })
                };
                stray || bare || p.iter().enumerate().any(list)
            }
            ".print" => self.lead_of_two_names(p),
            _ => false,
        }
    }

    /// The words of the line `first..=last` as Xyce cuts a D, V or I line,
    /// its name included: at a blank, a comma and a parenthesis
    /// (`D1 (a 0) DM`, `V1 a,0`, `V1(a 0)`: Xyce 7.10 reads each; an R, L
    /// or C keeps the comma and the parenthesis as fields).
    fn words(&self, first: u32, last: u32) -> usize {
        assert!(first <= last, "a line has its head");
        let cut = |c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')');
        let line = self.span(first, last);
        let n = line.split(cut).filter(|w| !w.is_empty()).count();
        assert!(n <= line.len(), "a word holds a byte");
        n
    }

    /// A spelling no Xyce line reads: `{POLY` with no `(n)` ("Syntax
    /// error in number of nodes"), a number whose exponent is a brace
    /// expression (`2.0e+{x}`, "Invalid notation").
    fn xyce_spelling(&self, first: u32, last: u32) -> bool {
        assert!(first <= last, "a line has its head");
        (first..last).any(|k| match self.ttag(k) {
            T::LBrace => {
                self.t.text(k + 1).eq_ignore_ascii_case("poly")
                    && (k + 2 > last || self.ttag(k + 2) != T::LParen)
            }
            T::Number => {
                let touch = |j: u32| j <= last && self.t.end(j - 1) == self.t.start(j);
                let sign = matches!(self.ttag(k + 1), T::Plus | T::Minus) && touch(k + 1);
                let brace = k + 1 + u32::from(sign);
                let n = self.t.text(k).to_ascii_lowercase();
                (n.ends_with('e') || n.ends_with("e+") || n.ends_with("e-"))
                    && touch(brace)
                    && self.ttag(brace) == T::LBrace
            }
            _ => false,
        })
    }

    /// A `.PRINT` brace expression holding a lead current of two
    /// blank-separated names (`{I2(YPDE BJT)}`: "Unrecognized character";
    /// outside braces Xyce reads the two words as one device name).
    fn lead_of_two_names(&self, p: &[(u32, u32)]) -> bool {
        p.iter().any(|&(a, b)| {
            assert!(a <= b, "a field has its first token");
            self.ttag(a) == T::LBrace
                && (a..b).any(|k| {
                    let lead = self.ttag(k) == T::Ident
                        && self.t.text(k).starts_with(['i', 'I'])
                        && self.ttag(k + 1) == T::LParen
                        && self.t.end(k) == self.t.start(k + 1);
                    lead && k + 3 <= b
                        && self.ttag(k + 2) == T::Ident
                        && self.ttag(k + 3) == T::Ident
                })
        })
    }

    /// A PAT source Xyce rejects: TR, TF or TSAMPLE not positive, no DATA
    /// field or one not `b` and bits 0/1, an `RB` past 1 (Xyce RG, PAT
    /// source; `-norun` on each: `RB=0` and `RB=-1` run).
    fn pat_rejected(&self, first: u32, last: u32) -> bool {
        let Some(kw) = (first + 1..=last)
            .find(|&k| self.ttag(k) == T::Ident && self.t.text(k).eq_ignore_ascii_case("pat"))
        else {
            return false;
        };
        let (mut nums, mut data, mut k) = (Vec::new(), None, kw + 1);
        // `DATA=`: the fields are given by name, not by place.
        let mut named = false;
        // Each pass moves `k` on by at least one token.
        while k <= last {
            match self.ttag(k) {
                T::LBrace | T::Quote => return false,
                T::Ident if k < last && self.ttag(k + 1) == T::Equal => {
                    let key = self.t.text(k);
                    if key.eq_ignore_ascii_case("rb")
                        && k + 2 <= last
                        && self.ttag(k + 2) == T::Number
                        && self.t.value(k + 2) > 1.0
                    {
                        return true;
                    }
                    named |= key.eq_ignore_ascii_case("data");
                    k += 2;
                }
                T::Minus if k < last && self.ttag(k + 1) == T::Number => {
                    nums.push(-self.t.value(k + 1));
                    k += 1;
                }
                T::Number if nums.len() < 6 => nums.push(self.t.value(k)),
                T::Number | T::Ident if nums.len() == 6 && data.is_none() => data = Some(k),
                // A name where a number goes (a parameter, another wave's
                // word after a node named `pat`): no plain PAT.
                T::Ident if nums.len() < 6 => return false,
                _ => {}
            }
            k += 1;
        }
        assert!(k > last, "the line is read to its end");
        let not_positive = nums.get(3..6).is_some_and(|t| t.iter().any(|&v| v <= 0.0));
        let no_data = nums.len() == 6 && data.is_none() && !named;
        let bits = |d: u32| {
            let t = self.t.text(d).to_ascii_lowercase();
            self.ttag(d) == T::Ident
                && t.len() > 1
                && t.starts_with('b')
                && t[1..].bytes().all(|c| c == b'0' || c == b'1')
        };
        not_positive || no_data || data.is_some_and(|d| !bits(d))
    }

    /// The pieces of `p` that are no `key=value` (nor a value after a
    /// `key =` or after a piece ending in `,`): the positional fields.
    fn positional(&self, p: &[(u32, u32)]) -> Vec<(u32, u32)> {
        assert!(
            p.iter().all(|&(a, b)| a <= b),
            "a field has its first token"
        );
        let mut out = Vec::new();
        let mut joined = false;
        for (i, &(a, b)) in p.iter().enumerate() {
            let keyed = (a..=b).any(|k| self.ttag(k) == T::Equal)
                || p.get(i + 1).is_some_and(|&(c, _)| self.ttag(c) == T::Equal);
            if !(joined || keyed) {
                out.push((a, b));
            }
            joined = matches!(self.ttag(b), T::Equal | T::Comma);
        }
        assert!(
            out.len() <= p.len(),
            "a positional field is one of the fields"
        );
        out
    }

    /// ngspice-45 stops on a PWL `r=` at or past the last time point or
    /// not one of the wave's time points (a negative `r` repeats nothing).
    fn ngspice_rejects(&self, first: u32, last: u32) -> bool {
        if !matches!(
            self.t.text(first).as_bytes()[0].to_ascii_uppercase(),
            b'V' | b'I'
        ) {
            return false;
        }
        let Some(w) = self.pwl(first, last) else {
            return false;
        };
        let times = w.times();
        match (w.repeat, times.last(), w.expr) {
            (Some(r), Some(&end), false) if r >= 0.0 => {
                // ponytail: equal to 1e-9 relative, as two spellings of one
                // time (`1u`, `1e-6`) parse; ngspice compares the doubles.
                let at = |t: f64| (r - t).abs() <= 1e-9 * t.abs().max(r.abs());
                r >= end || !times.iter().any(|&t| at(t))
            }
            _ => false,
        }
    }

    /// The PWL after a `pwl` word of the line `first..=last`, its values
    /// as written; `None` when the line has no `pwl`.
    fn pwl(&self, first: u32, last: u32) -> Option<Pwl> {
        let kw = (first + 1..=last)
            .find(|&k| self.ttag(k) == T::Ident && self.t.text(k).eq_ignore_ascii_case("pwl"))?;
        let mut w = Pwl {
            points: Vec::new(),
            repeat: None,
            expr: false,
            file: false,
        };
        // A number, `-` number, at `k`: its value and the token after it.
        let num = |k: u32| match self.ttag(k) {
            T::Number => Some((self.t.value(k), k + 1)),
            T::Minus if k < last && self.ttag(k + 1) == T::Number => {
                Some((-self.t.value(k + 1), k + 2))
            }
            _ => None,
        };
        let mut k = kw + 1;
        // Each pass moves `k` on by at least one token.
        while k <= last {
            let key = k < last && self.ttag(k) == T::Ident && self.ttag(k + 1) == T::Equal;
            if key {
                let name = self.t.text(k).to_ascii_lowercase();
                match (k + 2 <= last).then(|| num(k + 2)).flatten() {
                    Some((v, next)) => {
                        if name == "r" {
                            w.repeat = Some(v);
                        }
                        k = next;
                    }
                    None => {
                        w.expr |= name == "r";
                        k += 2;
                    }
                }
                continue;
            }
            match (self.ttag(k), num(k)) {
                (_, Some((v, next))) => {
                    w.points.push(v);
                    k = next;
                    continue;
                }
                (T::LParen | T::RParen | T::Comma, None) => {}
                (T::Ident, None) if self.t.text(k).eq_ignore_ascii_case("file") => w.file = true,
                (T::LBrace | T::Quote | T::Ident | T::String, None) => w.expr = true,
                _ => {}
            }
            k += 1;
        }
        Some(w)
    }

    /// The fields of the line after its head, `first..=last`: runs of
    /// tokens with no blank between them, a parenthesis's, brace's or
    /// quote's inside one field. The tokens touching the head are its name
    /// (Xyce names hold `#`, `[`, `+`, `` ` ``: Xyce_Regression
    /// INVALID_CHARS/valid_chars_*.cir).
    fn pieces(&self, first: u32, last: u32) -> Vec<(u32, u32)> {
        let touching = |k: u32| self.t.end(k - 1) == self.t.start(k);
        let mut start = first + 1;
        while start <= last && touching(start) {
            start += 1;
        }
        let mut out: Vec<(u32, u32)> = Vec::new();
        let (mut depth, mut quoted) = (0u32, false);
        for k in start..=last {
            match out.last_mut() {
                Some(p) if touching(k) || depth > 0 || quoted => p.1 = k,
                _ => out.push((k, k)),
            }
            match self.ttag(k) {
                T::LParen | T::LBrace => depth += 1,
                T::RParen | T::RBrace => depth = depth.saturating_sub(1),
                T::Quote => quoted = !quoted,
                _ => {}
            }
        }
        assert!(
            out.iter().all(|&(a, b)| a <= b && b <= last),
            "pieces in the line"
        );
        out
    }
}
