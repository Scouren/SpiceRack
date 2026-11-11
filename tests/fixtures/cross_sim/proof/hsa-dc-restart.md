# S-HSA-DC-RESTART: ngspice's hsa mode restarts every dc point (source)

ngspice-45 `src/spicelib/analysis/dctrcurv.c:302-308`: with `newcompat.hs` (`ngbehavior=hsa`, the
harness's stand-in for HSPICE) each point of a `.dc` sweep is a fresh operating point
(`CKTop(ckt, ... MODEINITJCT, ... MODEINITFLOAT, ckt->CKTdcMaxIter)`); without it a point continues
from the last one (`NIiter`), and `CKTop` runs only when that fails. A circuit with more than one
dc solution (a Schmitt trigger inside its hysteresis, HICUM's `fout` decks past breakdown) then
lands on the solution a cold start reaches, not on the branch the sweep is on.

Runs, 2026-11-11, ngspice-45, the dc plot compared point by point as `tests/test_cross_sim/cmp.rs`
does (2 * (RELTOL * swing + floor)); "written" is the HSPICE deck of golden record fx17
(p7-integrate 1298f0b), "hsa" is `ngspice -D ngbehavior=hsa`:

| deck | ngspice: written against original | hsa against ngspice, both on the original | hsa: written against original |
|---|---|---|---|
| circuitsim90/MOS2/schmitfast.sp | equal (difference 0) | v(6) at 2.298 V: 6.2873e-5 against 4.999997 | equal |
| circuitsim90/MOS2/schmitslow.sp | equal (difference 0) | v(6) at 2.403 V: 0.786075 against 4.224771 | equal |
| cmcqa/hicum/hicum_fout_npn_1D_npn_full.sp | 0 of 79 vectors out (worst 2.1e-9 of the tolerance) | 11 of 79 out; i(vemit_3) at 4.55 V: 0.0839569 against 2.18411e-4 | 6 of 79 out; i(vemit_4) at 4.45 V: 0.0936453 against 0.0149418 |
| cmcqa/hicum/hicum_fout_npn_1D_npn_internal.sp | 0 of 65 out (1.7e-10) | 10 of 65 out; i(vemit_1) at 5.9 V: 0.314792 against 5.24569e-6 | 0 of 65 out (7.9e-14) |
| cmcqa/hicum/hicum_fout_npn_1D_npn_lat_nqs.sp | 0 of 65 out (2.5e-10) | 10 of 65 out; i(vemit_1) at 5.85 V: 0.307942 against 5.24396e-6 | 4 of 65 out; i(vemit_1) at 5.55 V: 5.23396e-6 against 0.280204 |

Column 2: the written deck is the original's circuit (plain ngspice reads both alike). Column 3:
the mode moves the ORIGINAL deck off its own plain result at the listed vectors, so the listed
mismatch is the mode's, not the writer's. Column 4: on the two decks where the mode does not
repeat itself between the two spellings of one circuit, the restart lands on either solution
(the written sweep step is `4.9999999999999996e-2` for the original's `0.050000`, one unit in the
last place, which plain ngspice does not feel).

Source: research/settle-ngspice.md (hsa schmitfast/slow, hicum_fout); ZERO-LEDGER P1-045.
