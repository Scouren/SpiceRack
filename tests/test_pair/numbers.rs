//! Step 4 of a cell over two simulators (`rewrite/SPINE.md` §8a): the source simulator's run
//! against the target's on the written text, each held in the other's time tube at
//! `common/cmp.rs`'s tolerances; a transient that differs is run again with both decks refined
//! (`simulator-differs:refinement` when they agree there, rulings R84, R84a), else `mismatch`.
//!
//! Cites: rulings R84 (the refinement class), R84a (its tube); `common/cmp.rs`.
//!
//! Contents:
//! - `pub fn step(c: &Cell, deck: &Deck, pair: &PairFile, (uic, renames), (text, written, rest), (a, b)) -> Option<Fail>`

use std::collections::HashMap;

use spicerack::result::RawData;

use crate::files::PairFile;
use crate::run::{Deck, Fail, SECS, fail};
use crate::sim::{self, Ran};
use crate::{Cell, cmp, plots};

/// The refinement oracle's step cap: a transient's span over this (a deck's own smaller cap
/// kept). span/2000 was too coarse once the tube narrowed: the MOS level 3 inverter's gate
/// current during its input ramp is 1.2 % apart at the deck's own 1 ns cap and 0.25 % at 0.1 ns
/// (both simulators moving toward each other), so only a finer cap shows its convergence.
const REFINE: f64 = 20000.0;

/// The refined comparison's time tube, a fraction of the stop time: ten refined steps.
const FINE_TUBE: f64 = 10.0 / REFINE;

/// Step 4 of a cell over two simulators: the two runs agree in `cmp`'s tolerances, or agree
/// once both are refined (`simulator-differs:refinement`), or `mismatch`.
pub fn step(
    c: &Cell,
    deck: &Deck,
    pair: &PairFile,
    (uic, renames): (bool, &HashMap<String, String>),
    (text, written, rest): (&str, &str, &[(String, String)]),
    (a, b): (&[RawData], &[RawData]),
) -> Option<Fail> {
    let mut tol = cmp::Tol::of(&[text, written]);
    for (k, v) in &pair.tightened {
        tol.tighten(k, *v).expect("pair_file checked it");
    }
    match agree(a, b, tol) {
        Err(why) => {
            // The refinement oracle (cross-sim's `simulator-differs:refinement`): both decks
            // with every transient's step capped at span/REFINE; agreeing there, with the time
            // tube narrowed to FINE_TUBE (rulings R84a: the tube absorbs an edge's placement by
            // the steps, never a difference the circuit keeps), the two simulators' step
            // control made the difference, not the conversion. F52 showed the hole: a reverse
            // recovery 4.8 ns later on VACASK with both steps at 0.5 ns, inside the 1 % tube
            // (10 ns).
            let mut fine = tol;
            fine.tighten("tube", FINE_TUBE)
                .expect("FINE_TUBE is under the default tube");
            let refined = sim::refined(c.from_sim, text, REFINE)
                .zip(sim::refined(c.to_sim, written, REFINE))
                .and_then(|(ra, rb)| {
                    let a2 = sim::run_checked(c.from_sim, &deck.path, &ra, &[], SECS);
                    let b2 = sim::run_checked(
                        c.to_sim,
                        &deck.path.with_extension(c.to_ext),
                        &rb,
                        rest,
                        SECS,
                    );
                    match (a2, b2) {
                        (Ran::Plots(mut x), Ran::Plots(mut y)) => Some({
                            if uic {
                                cmp::without_start(&mut x);
                                cmp::without_start(&mut y);
                            }
                            agree(
                                &plots::only(plots::hierarchical(x), pair.vectors.as_ref()),
                                &plots::only(
                                    plots::renamed(plots::hierarchical(y), renames),
                                    pair.vectors.as_ref(),
                                ),
                                fine,
                            )
                        }),
                        _ => None,
                    }
                });
            let f = match refined {
                Some(Ok(())) => fail(
                    "simulator-differs:refinement",
                    format!("{why}; both agree with every step capped at span/{REFINE}"),
                ),
                Some(Err(still)) => fail(
                    "mismatch",
                    format!("{why}; with every step capped at span/{REFINE}: {still}"),
                ),
                None => fail("mismatch", why),
            };
            Some(f)
        }
        Ok(()) => {
            if std::env::var_os("PAIR_MARGINS").is_some() {
                eprintln!("margin {}: {}", deck.name, margin(a, b, text, written));
            }
            None
        }
    }
}

// ---- private ----

/// Whether the two runs agree each way: `cmp::compare` holds the first run in the second's
/// time tube only, so a target that rings about the source's value passed it (F51: ngspice's
/// ±1 A trapezoidal ringing about VACASK's -1e-7 A held VACASK's run inside its band).
fn agree(a: &[RawData], b: &[RawData], tol: cmp::Tol) -> Result<(), String> {
    cmp::compare(a, b, tol)?;
    cmp::compare(b, a, tol).map_err(|why| format!("the target's run in the source's tube: {why}"))
}

/// How much of step 4's value tolerance a passing pair uses (`PAIR_MARGINS=1`, triage). It
/// measures the value band only: on a smooth transient the 1 % time tube covers the difference,
/// so "passes at 0" there means not measured by this probe, never 0 %. Reports the smallest
/// fraction `f` of RELTOL, VNTOL and ABSTOL (the transient's band scales with them) at which
/// `compare` still passes, found by bisection to 1 %, and the difference `compare` names just
/// below it (the worst vector, its value and the tolerance there).
fn margin(a: &[RawData], b: &[RawData], src: &str, written: &str) -> String {
    let at = |f: f64| cmp::compare(a, b, cmp::Tol::of(&[src, written]).scaled(f));
    let (mut lo, mut hi) = (0.0f64, 1.0f64);
    // Bounded: seven halvings take the bracket under 1 %.
    for _ in 0..7 {
        let mid = (lo + hi) / 2.0;
        match at(mid) {
            Ok(()) => hi = mid,
            Err(_) => lo = mid,
        }
    }
    let below = at(lo).err().unwrap_or_else(|| {
        "passes at 0: not measured by this probe (exact, or inside the time tube)".into()
    });
    format!(
        "uses {:.0} % of the tolerance; below it: {below}",
        hi * 100.0
    )
}
