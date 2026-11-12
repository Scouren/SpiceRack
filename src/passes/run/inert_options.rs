//! Rule `inert_options`: an option that changes no result dropped, one note each
//! (`OPTION_DROPPED`, its argument the key's index). One row per key, each removing its
//! `Option(<key>)` and requiring nothing: `noacct`, `acct`, `list`, `nomod`, `nopage`,
//! `node`, `opts`, `warn`, `maxwarns`, `norefvalue`, `noinit`, `nfreqs`, `fourgridsize`
//! (rows `drop_option_<key>`).
//!
//! Exact by runs, never by a manual (`tests/passes_run/decks/options_probe.sp`: a pulse into an
//! RC, a diode through an inductor to a load, a level-1 MOSFET with no W, L, AD or AS; op, dc,
//! ac, tran, and a `.four`): ngspice-45 `-b -r` writes the same raw file with each of these
//! options as without it (the `Date:` line aside); `nfreqs` and `fourgridsize` change only the
//! log's Fourier table. `tests/passes_run/options.rs` reruns the table. The same runs leave out:
//!
//! | option | what changed in the raw file |
//! |---|---|
//! | `keepopinfo` | an extra plot, the AC's operating point |
//! | `interp` | the transient's points (243 → 201, resampled) |
//! | `autostop` | the transient stops at its first point (243 → 1) |
//! | `trtol=10`, `method=gear` | the transient's step count (243 → 246) |
//! | `rshunt=1e12` | values (i(vdd) by 0.67 relative in dc and tran) |
//! | `cshunt=1e-15` | values (ac i(vdd) 6e-2) and the transient's step count |
//! | `rseries=1e-3` | an extra node (`l1_intern__`) and values |
//! | `defl`, `defw`, `defad`, `defas` | values (MOSFET geometry) |
//! | `klu` | values at 1e-7 relative (another solver's rounding) |
//!
//! and `seed=7`, `pivtol`, `pivrel`, `itl2`, `sparse`, identical here, which the deck does not
//! exercise (no random draw, no near-singular pivot, no DC point past the default iterations,
//! the default solver): not proven inert, so no row.
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
    let mut drop = Vec::new();
    for &(item, scope) in &reachable {
        if !carriers.contains(&item) {
            continue;
        }
        // an option in a loop body or a branch is left: dropping it changes the body's shape
        // (a loop of one op, `LoopOverOp`), a feature of its own
        let top = design.scope(scope).kind == ir::ScopeKind::File;
        if item.kind == ir::ItemKind::Option && top {
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
