# S-BSIM3-DEFAULT-CAPMOD — third-simulator (ngspice-45 is the outlier)

The BSIM3 comparator family (`.model nmos nmos (level=9)`, every parameter
default), the floating stack node v(1) at 11 / 20 / 41 / 55 ns:

| simulator | v(1) |
|---|---|
| LTspice 26.1.1 (BSIM3v3.3 level 8, defaults) | -0.3749 / -0.3056 / -0.3750 / -0.2948 V |
| Xyce 7.10 | -0.375 / -0.306 / -0.375 / -0.295 V |
| ngspice-45 | +0.882 / +0.882 / +0.900 / +0.900 V |

Two of three agree; ngspice-45's BSIM3 3.3 (default capmod=3) is the outlier.
No writer change (team-lead ruling 2026-11-07: `version=3.1` matches only by
accident, BSIM3v1 defaults capmod=2).

Five decks first admitted here (BUG_1797 one-shot_lev9, BUG_381
comparator0_5_0 and comparator1, IC_AND_NODESET/ic_comp, ONESHOT/one-shot)
pass the harness's refinement oracle since its TSTOP/20000 run (e47042d):
their difference was the step, so they are `refinement` lines, not this
rule's.

Seven more decks are the family's circuit behind binned model cards
(`.model nmos.1 nmos (level=9 lmin=… wmax=…)`, every other parameter
default) or `.options parser scale`: `MODEL_BINNING/comparatorBinning.cir`,
`comparatorBinning_forGlobal.cir`, `globalPar_comparatorBinning.cir`,
`BUG_1041_SON/bsim3_comparatorBinningScale.cir` and `…ScaleRef.cir`,
`ISSUE_239/bsim3_test2.cir` and `bsim3_test2Ref.cir`. Each one's Xyce run
against its baseline's Xyce run, and its ngspice run against its baseline's
ngspice run, compared as `tests/test_cross_sim/cmp.rs` compares a pair:
every shared vector inside the tube (worst distance from the band 0 of the
tolerance, both simulators, all seven). Each leaves the tube against the
other simulator at the baseline's sample: V(1) = -0.3762851594665437 V on
Xyce (the same 16 digits as `comparatorBaseline.cir`) where ngspice has
+0.88245705936 V, t = 10.9548 ns. `bsim3_comparatorBinningScale2.cir` is
another circuit on both simulators (v(8) 99 tolerances from the baseline's)
and is not this rule's. Run 2026-11-11, Xyce 7.10.0 and ngspice-45
(ZERO-LEDGER P1-066).

Source: research/mismatches.md §2 (S-BSIM3-DEFAULT-CAPMOD); ZERO-LEDGER P1-032.
