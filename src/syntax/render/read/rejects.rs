//! Lines a source's own simulator rejects before it runs the deck: such a
//! line has no meaning to write, so the reader leaves it out with a note
//! naming the cause (`P0010` Xyce, `P0011` an ngspice PWL `r=`). Only the
//! classes visible from the line itself are here; a deck its simulator
//! rejects for another file or for what two lines mean together is named
//! in `tests/fixtures/conformance_noise.txt`.
//!
//! Cites: Xyce 7.10 `-norun` on each class (the messages in `P0010`'s
//! cite); ngspice-45 runs (`P0011`'s cite, ngspice manual §4.1.4).
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
    /// Xyce 7.10 `-norun` rejects the line `first..=last`: an analysis
    /// card's field count, an R/L/C with fewer than two nodes or a second
    /// bare value, a PWL with no points or `r=` outside `[0, t_last)`.
    fn xyce_rejects(&self, first: u32, last: u32) -> bool {
        let head = self.t.text(first).to_ascii_lowercase();
        let p = self.pieces(first, last);
        let word = |i: usize| p.get(i).map(|&(a, b)| self.span(a, b).to_ascii_lowercase());
        let sweep =
            |w: Option<String>| w.is_some_and(|w| matches!(w.as_str(), "dec" | "oct" | "lin"));
        match head.as_str() {
            ".tran" => return p.len() < 2,
            ".fft" => return p.is_empty(),
            ".four" => return p.len() < 2,
            ".ac" => return sweep(word(0)) && p.len() != 4,
            // `.DC v start stop step …`: four fields per linear sweep.
            ".dc" => {
                let keyed = (0..p.len()).any(|i| {
                    let w = word(i).expect("i < p.len()");
                    sweep(Some(w.clone()))
                        || w == "list"
                        || w.contains('=')
                        || w.starts_with("data")
                });
                return !keyed && !p.len().is_multiple_of(4);
            }
            // `.NOISE out src type n fstart fstop [pts]`.
            ".noise" => {
                return (0..p.len())
                    .find(|&i| sweep(word(i)))
                    .is_some_and(|i| !matches!(p.len() - i, 4 | 5));
            }
            _ => {}
        }
        // `L=1E-4 …`: a continuation's keys cut off by a comment line
        // (Xyce_Regression BUG_250, which Xyce runs), no element.
        if first < last && self.ttag(first + 1) == T::Equal {
            return false;
        }
        match head.as_bytes()[0].to_ascii_uppercase() {
            b'R' | b'L' | b'C' => {
                let bare = |i: usize| {
                    p.get(i)
                        .is_some_and(|&(a, b)| a == b && self.ttag(a) == T::Number)
                };
                p.len() < 2 || (bare(2) && bare(3))
            }
            b'V' | b'I' => self.pwl(first, last).is_some_and(|w| {
                let times = w.times();
                let no_points = w.points.is_empty() && !w.expr && !w.file;
                let off = match (w.repeat, times.last(), w.expr) {
                    (Some(r), Some(&end), false) => r < 0.0 || r >= end,
                    _ => false,
                };
                no_points || off
            }),
            _ => false,
        }
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
