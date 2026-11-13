//! Rule `inert_options`: an option that changes no result dropped, one note each
//! (`OPTION_DROPPED`, its argument the key's index). One row per key, each removing its
//! `Option(<key>)` and requiring nothing: `noacct`, `acct`, `list`, `nomod`, `nopage`,
//! `node`, `opts`, `warn`, `maxwarns`, `norefvalue`, `noinit`, `nfreqs`, `fourgridsize`, and
//! the two solver selections `klu`, `sparse`, and the step-selection hint `trtol` (rows
//! `drop_option_<key>`; R83, R84).
//!
//! Exact by runs, never by a manual (`tests/passes_run/decks/options_probe.sp`: a pulse into an
//! RC, a diode through an inductor to a load, a level-1 MOSFET with no W, L, AD or AS; op, dc,
//! ac, tran, and a `.four`): ngspice-45 `-b -r` writes the same raw file with each of the
//! first thirteen as without it (the `Date:` line aside); `nfreqs` and `fourgridsize` change
//! only the log's Fourier table. The solver selections within a stated bound (R60): `sparse` is
//! the default solver and identical; `klu` moves values by at most 1e-7 relative (above ABSTOL,
//! 1e-12), another solver's rounding. `tests/passes_run/options.rs` reruns those fifteen. `trtol` is a hint (R84):
//! a factor on the truncation-error test picks steps only; set to VACASK's `tran_lteratio`
//! value it does not give both simulators the same steps (`pair`'s lossless-LC corpus decks
//! differ unrefined and agree refined, with and without it), so it is never mapped, and the
//! pair test's refinement oracle is its evidence.
//!
//! An option in a loop's body or a branch, or one a loop or alter targets, is left with
//! `OPTION_KEPT`: dropping it changes the body's shape (a loop of one op, `LoopOverOp`). So is
//! every option an IR row reads, whatever the raw file says: `nfreqs` in a design with a Fourier
//! analysis that has no harmonic count of its own (`Four::harmonics`; `state_order::four_counts`
//! checks one is in force: dropping it failed `ir::check`, `round9`'s finding on ir-next
//! 43a034a4), `fourgridsize` in one with a Fourier analysis that has no grid (`Four::grid`).
//! No IR row reads the other thirteen keys (a grep of `crates/ir` for each).
//!
//! Not dropped, and why (R83; the same runs; each owed until a census deck is freed by it):
//!
//! | option | what the run showed | why no row |
//! |---|---|---|
//! | `rshunt=1e12` | values (i(vdd) by 0.67 relative in dc and tran) | meaning: explicit elements |
//! | `cshunt=1e-15` | values (ac i(vdd) 6e-2), the transient's step count | meaning |
//! | `rseries=1e-3` | an extra node (`l1_intern__`) and values | meaning |
//! | `defl`, `defw` | values (MOSFET current, 0.98 relative) | meaning: instance fields |
//! | `defad`, `defas` | values (ac 3e-3, tran 5e-2) | meaning: instance fields |
//! | `interp` | the transient's points (243 → 201, resampled) | the print step as an output interval (R67) |
//! | `keepopinfo` | an extra plot, the AC's operating point | output shape |
//! | `autostop` | the transient stops at its first point (243 → 1) | output shape |
//! | `method=gear` | the transient's step count (243 → 246) | the integration method is meaning where the target states it (R84) |
//! | `seed=7`, `pivtol`, `pivrel`, `itl2` | identical | the deck does not exercise them (no draw, no near-singular pivot, no DC point near the iteration limit) |
//!
//! Cites: `crates/ir/src/option_key.rs` (`OptionClass::Output`); R67.
//!
//! Contents: `pub fn drop_option(&mut ir::Design, &mut ir::Diags, ir::OptionKey)`,
//! `pub type Row`, `pub const INERT: &[Row]`,
//! `pub const OPTION_DROPPED: ir::Code`, `pub const OPTION_KEPT: ir::Code`

use crate::ir;
use ir::OptionKey as K;

/// The note at a dropped option's place: `arg` is the key's index in `OptionKey::ALL`.
pub const OPTION_DROPPED: ir::Code = ir::Code::new(b'U', 420);
/// The note at an item the rule leaves carrying the key: `arg` is the key's index.
pub const OPTION_KEPT: ir::Code = ir::Code::new(b'U', 421);

/// A row: its name, its key, its run function.
pub type Row = (
    &'static str,
    ir::OptionKey,
    fn(&mut ir::Design, &mut ir::Diags),
);

/// The keys proven inert, with each row's name and run function.
pub const INERT: &[Row] = &[
    ("drop_option_noacct", K::NoAcct, |d, g| {
        drop_option(d, g, K::NoAcct)
    }),
    ("drop_option_acct", K::Acct, |d, g| {
        drop_option(d, g, K::Acct)
    }),
    ("drop_option_list", K::List, |d, g| {
        drop_option(d, g, K::List)
    }),
    ("drop_option_nomod", K::NoMod, |d, g| {
        drop_option(d, g, K::NoMod)
    }),
    ("drop_option_nopage", K::NoPage, |d, g| {
        drop_option(d, g, K::NoPage)
    }),
    ("drop_option_node", K::Node, |d, g| {
        drop_option(d, g, K::Node)
    }),
    ("drop_option_opts", K::Opts, |d, g| {
        drop_option(d, g, K::Opts)
    }),
    ("drop_option_warn", K::Warn, |d, g| {
        drop_option(d, g, K::Warn)
    }),
    ("drop_option_maxwarns", K::MaxWarns, |d, g| {
        drop_option(d, g, K::MaxWarns)
    }),
    ("drop_option_norefvalue", K::NoRefValue, |d, g| {
        drop_option(d, g, K::NoRefValue)
    }),
    ("drop_option_noinit", K::NoInit, |d, g| {
        drop_option(d, g, K::NoInit)
    }),
    ("drop_option_nfreqs", K::NFreqs, |d, g| {
        drop_option(d, g, K::NFreqs)
    }),
    ("drop_option_fourgridsize", K::FourGridSize, |d, g| {
        drop_option(d, g, K::FourGridSize)
    }),
    ("drop_option_klu", K::Klu, |d, g| drop_option(d, g, K::Klu)),
    ("drop_option_sparse", K::Sparse, |d, g| {
        drop_option(d, g, K::Sparse)
    }),
    ("drop_option_trtol", K::TrTol, |d, g| {
        drop_option(d, g, K::TrTol)
    }),
];

/// Drops every reachable option row of `key` listed in a file's top scope, a note at each; notes
/// every other item that carries the key.
pub fn drop_option(design: &mut ir::Design, diags: &mut ir::Diags, key: ir::OptionKey) {
    assert!(INERT.iter().any(|r| r.1 == key), "{key:?} is proven inert");
    let index = ir::OptionKey::ALL
        .iter()
        .position(|&k| k == key)
        .expect("a key of ALL") as u32;
    let feature = ir::Feature::Option(key);
    let mut rows = ir::Diags::new();
    ir::check_legal(
        design,
        &ir::FeatureSet::ALL.minus(ir::FeatureSet::EMPTY.with(feature)),
        &mut rows,
    );
    let carriers: Vec<ir::ItemRef> = rows.rows().iter().map(|d| d.item).collect();
    let reachable = design.reachable();
    // an option an IR row reads is never inert there: a Fourier analysis with no harmonic count
    // takes it from `nfreqs`, one with no grid from `fourgridsize` (`Four::harmonics`,
    // `Four::grid`); no row reads the other keys (`crates/ir`: grep of the keys)
    let read = reachable.iter().any(|&(i, _)| {
        i.kind == ir::ItemKind::Analysis && {
            let a = design.row::<ir::Analysis>(i.index);
            a.kind == ir::AnalysisKind::Four && {
                let f = design.detail::<ir::tran::Four>(a.detail);
                match key {
                    K::NFreqs => f.harmonics.is_none(),
                    K::FourGridSize => f.grid.is_none(),
                    _ => false,
                }
            }
        }
    });
    let mut drop = Vec::new();
    for &(item, scope) in &reachable {
        if !carriers.contains(&item) {
            continue;
        }
        // an option in a loop body or a branch is left: dropping it changes the body's shape
        // (a loop of one op, `LoopOverOp`), a feature of its own
        let top = design.scope(scope).kind == ir::ScopeKind::File;
        if item.kind == ir::ItemKind::Option && top && !read {
            diags.push_at(OPTION_DROPPED, design, item, index);
            drop.push((item, scope));
        } else {
            diags.push_at(OPTION_KEPT, design, item, index);
        }
    }
    let mut editor = design.editor();
    for (item, scope) in drop {
        editor.remove_item(scope, item);
    }
}
