//! Acme, `DIALECT-PLUGIN.md` §2's small dialect, declared in Rust from
//! public items only: the definition the plugin tests (`test_dialect_plugin.rs`)
//! and acme's oracles (`test_acme_oracles.rs`) share, each by `#[path]` (not `mod common`).
//!
//! Cites: none (helpers).
//!
//! Contents:
//! - `pub static NOISE_CALLS: AtomicUsize`, `pub static NOISE_CARDS: AtomicUsize`
//! - `pub fn acme_as(name: &str) -> DialectDef`

use std::sync::atomic::{AtomicUsize, Ordering};

use spicerack::syntax::Lang;
use spicerack::syntax::behaviour::NumberReading;
use spicerack::syntax::def::{Answer, Card, DialectDef};
use spicerack::syntax::diag::Kind;
use spicerack::syntax::lexicon::{Arithmetic, Scale};
use spicerack::syntax::render::capability::{Alt, Construct};
use spicerack::syntax::render::spelling::FuncForm;
use spicerack::syntax::token::Tag;

/// How often [`write_noise`] was called, and with how many cards (read
/// by `test_dialect_plugin.rs` only).
#[allow(dead_code)]
pub static NOISE_CALLS: AtomicUsize = AtomicUsize::new(0);
#[allow(dead_code)]
pub static NOISE_CARDS: AtomicUsize = AtomicUsize::new(0);

/// Acme (§2) under `name`: ngspice with `#` comment lines and `;` tails,
/// `.subcircuit`/`.endsub`, `x` for mega, decimal arithmetic, functions as
/// `.param f(x)='body'`, no Laplace sources (its own code), charge
/// capacitors braced, `reltol` spelled `rtol`, and a `.noise` hook.
pub fn acme_as(name: &str) -> DialectDef {
    DialectDef::new(name)
        .extends(Lang::NGSPICE)
        .comment_line("#")
        .comment_tail(";")
        .keyword(".subcircuit", Tag::KwSubckt)
        .keyword(".endsub", Tag::KwEnds)
        .write_head(Tag::KwSubckt, ".subcircuit")
        .write_head(Tag::KwEnds, ".endsub")
        .scale("x", Scale::Exp(6))
        .arithmetic(Arithmetic::Decimal)
        .number_reading(NumberReading::Decimal)
        .ngspice_numbers(false)
        .func(FuncForm::Param)
        .spell(Construct::Func, Alt::FuncParam)
        .option("reltol", "rtol")
        .code("ACME001", Kind::Refusal, "acme has no Laplace sources")
        .code("ACME002", Kind::Refusal, "acme's .noise takes one source")
        .refuse_code(Construct::Laplace, "ACME001")
        .spell(Construct::ChargeCapacitor, Alt::ChargeBraced)
        .hook(".noise", write_noise)
}

// ---- private ----

/// Acme's `.noise`: `.noise_acme out=<o> src=<s> <sweep>`; a card with
/// fewer than two words refused `ACME002`.
fn write_noise(cards: &[Card<'_>], out: &mut Vec<Answer>) {
    NOISE_CALLS.fetch_add(1, Ordering::SeqCst);
    NOISE_CARDS.fetch_add(cards.len(), Ordering::SeqCst);
    for c in cards {
        out.push(match c.words {
            [o, s, rest @ ..] => {
                Answer::Text(format!(".noise_acme out={o} src={s} {}", rest.join(" ")))
            }
            _ => Answer::Refuse("ACME002"),
        });
    }
}
