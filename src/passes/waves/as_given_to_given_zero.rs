//! Pass `as_given_to_given_zero`: a design in, the same design out with every wave under the
//! as-given rule (`GivenZero::AsGiven`) relabelled with ngspice's zero rule
//! (`GivenZero::SpiceDefault`) where the two read every field the same: the inverse of
//! `given_zero_to_as_given`, over IR-CIRCUIT §3's table (`wave_eval`'s defaults, A30). Under
//! both rules a given field means what it holds, except that ngspice replaces a *both* field
//! (PULSE rise, fall, width, period; SIN freq; EXP `td1`, `tau1`, `td2`, `tau2`; SFFM carrier)
//! that evaluates to zero with its default. So the label changes exactly when every *both*
//! field is a literal other than zero. A *both* field that is a literal zero (an ideal edge,
//! one pulse) or an expression that may evaluate to zero leaves the wave with note `U0205`.
//!
//! A field that is absent (`NONE`) under the as-given rule has no value there (`Undefined`:
//! the as-given rule has no defaults), where ngspice would read its default: that wave is left
//! with the same note, never given a value the source does not state.
//!
//! Removes `Circuit(GivenZeroAsGiven)`, requires `Circuit(GivenZeroSpiceDefault)`. One row: the
//! zero rule has no form.
//!
//! Cites: `rewrite/AMEND-A12.md` A17 (R35), A30; `rewrite/IR-CIRCUIT.md` §3;
//! `crates/ir/src/wave_eval.rs` (`Fields::omit`, `Fields::both`).
//!
//! Contents: `pub fn as_given_to_given_zero(&mut ir::Design, &mut ir::Diags)`

use super::GIVEN_ZERO_KEPT;
use super::nodes::{literal, note_left, sources};
use crate::ir;
use crate::ir::wave::{Am, Exp, GivenZero, Pulse, Sffm, Sin, TrRandom};

/// Relabels every as-given wave ngspice's zero rule reads the same; notes every other.
pub fn as_given_to_given_zero(design: &mut ir::Design, diags: &mut ir::Diags) {
    let waves: Vec<ir::WaveRef> = sources(design)
        .into_iter()
        .map(|(_, s)| s.wave)
        .filter(|w| !w.is_none())
        .collect();
    for w in waves {
        match w.kind {
            ir::WaveKind::Pulse => relabel::<Pulse>(design, w.index, pulse),
            ir::WaveKind::Sin => relabel::<Sin>(design, w.index, sin),
            ir::WaveKind::Exp => relabel::<Exp>(design, w.index, exp),
            ir::WaveKind::Sffm => relabel::<Sffm>(design, w.index, sffm),
            ir::WaveKind::Am => relabel::<Am>(design, w.index, am),
            ir::WaveKind::TrRandom => relabel::<TrRandom>(design, w.index, trrandom),
            ir::WaveKind::Pwl | ir::WaveKind::TrNoise | ir::WaveKind::Pattern => {}
        }
    }
    let feature = ir::Feature::Circuit(ir::CircuitExtra::GivenZeroAsGiven);
    note_left(design, diags, feature, GIVEN_ZERO_KEPT);
}

// ---- private ----

/// Reads the row at `index` and, when `rule` says both rules read it the same, stores it
/// under ngspice's rule.
fn relabel<D: ir::DetailRow + Copy>(
    design: &mut ir::Design,
    index: u32,
    rule: fn(&ir::Design, D) -> Option<D>,
) {
    let row: D = design.detail(index);
    if let Some(new) = rule(design, row) {
        design.editor().set_detail(index, new);
    }
}

fn pulse(d: &ir::Design, p: Pulse) -> Option<Pulse> {
    let both = [p.rise, p.fall, p.width, p.period];
    same(d, p.given_zero, &both, &[p.delay])?;
    Some(Pulse {
        given_zero: GivenZero::SpiceDefault,
        ..p
    })
}

fn sin(d: &ir::Design, s: Sin) -> Option<Sin> {
    same(
        d,
        s.given_zero,
        &[s.freq],
        &[s.delay, s.damping, s.phase_deg],
    )?;
    Some(Sin {
        given_zero: GivenZero::SpiceDefault,
        ..s
    })
}

fn exp(d: &ir::Design, x: Exp) -> Option<Exp> {
    same(d, x.given_zero, &[x.td1, x.tau1, x.td2, x.tau2], &[])?;
    Some(Exp {
        given_zero: GivenZero::SpiceDefault,
        ..x
    })
}

fn sffm(d: &ir::Design, f: Sffm) -> Option<Sffm> {
    let omit = [
        f.signal_freq,
        f.mod_index,
        f.delay,
        f.phase_c_deg,
        f.phase_m_deg,
    ];
    same(d, f.given_zero, &[f.carrier_freq], &omit)?;
    Some(Sffm {
        given_zero: GivenZero::SpiceDefault,
        ..f
    })
}

fn am(d: &ir::Design, a: Am) -> Option<Am> {
    let omit = [
        a.mod_index,
        a.mod_freq,
        a.carrier_freq,
        a.delay,
        a.phase_m_deg,
        a.phase_c_deg,
    ];
    same(d, a.given_zero, &[], &omit)?;
    Some(Am {
        given_zero: GivenZero::SpiceDefault,
        ..a
    })
}

fn trrandom(d: &ir::Design, r: TrRandom) -> Option<TrRandom> {
    same(d, r.given_zero, &[], &[r.td, r.p1, r.p2])?;
    Some(TrRandom {
        given_zero: GivenZero::SpiceDefault,
        ..r
    })
}

/// `Some` when the wave is as-given and both rules read it the same: every *both* field a
/// literal other than zero, every *omit* field given.
fn same(d: &ir::Design, rule: GivenZero, both: &[ir::ExprId], omit: &[ir::ExprId]) -> Option<()> {
    let nonzero = |&e: &ir::ExprId| literal(d, e).is_some_and(|v| v != 0.0);
    let given = |e: &ir::ExprId| !e.is_none();
    let ok = rule == GivenZero::AsGiven && both.iter().all(nonzero) && omit.iter().all(given);
    ok.then_some(())
}
