# HICUM-L2-AC-TM50 — source (ngspice-45 is the outlier against the CMC `.sp.reference`)

The eight HICUM/L2 ac decks at -50 °C of `cmcqa/hicum/` whose difference is
`i(vbase_9)`: `hicum_{CBC_CCS,CBE}_npn_{1D,1D_breakdown,1D_no_aval,vert_nqs}_ac_Tm50.sp`,
in the two ESPice cells (ngspice-espice, ngspice-hspice-espice).

Copy 9 of each deck (V(coll) = 0.8, ac 1 on the collector) at 10 GHz:
Im i(vbase_9) = ω·c(base,coll). Reference (`<deck>.reference` line 56, the
fifth 10 GHz row, the same in all eight): c(base,coll) = 2.9133337117754e-15 F,
so ω·c = 1.8305016e-4 A. ESPice 0.3.0: 1.8305015572738133e-4 (the reference
to 15 digits). ngspice-45: 1.8342421968148607e-4 (+0.204 %). Xyce 7.10 (the
ngspice-xyce list's own number for these decks): 1.8305016051898588e-4 (2.6e-8
from the reference). The real part: reference -g(base,coll) = 1.0e-12 S;
ESPice 1.0e-12, ngspice 2.0e-12.

ngspice's HICUM/L2 (level 8) c(base,coll) at -50 °C is the outlier. ESPice
gives the same numbers on the source deck and on the written one (the
harness's `source` kind): neither the translation nor ESPice is the cause.

Runs: 2026-11-11, ngspice-45 and ESPice 0.3.0 from `nix develop .#testing`,
`cmcqa/hicum/hicum_CBC_CCS_npn_1D_ac_Tm50.sp`. Re-derived from the reference
files by `tests/test_cross_sim/proofs.rs`.

Source: ZERO-LEDGER P3-103.
