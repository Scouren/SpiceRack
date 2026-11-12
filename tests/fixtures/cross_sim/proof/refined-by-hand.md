# S-NUMERIC-CONVERGES by hand: refinement certificates

Decks the harness's own refinement (TMAX (TSTOP-TSTART)/2000, then /20000, within its CPU budget)
leaves a `mismatch` and a finer step settles. Each row: the Xyce original on Xyce 7.10 and the deck
written for ngspice (golden record fx17, p7-integrate 1298f0b) on ngspice-45, both at the TMAX
named, every shared vector of the transient compared as `tests/test_cross_sim/cmp.rs` compares it
(1% of the swing plus twice the floor, through the 1% time tube). "worst" is the largest distance
from the band in units of the tolerance: under 1 passes. Run 2026-11-11.

| deck | TMAX | points Xyce / ngspice | at the harness's levels | at TMAX |
|---|---|---|---|---|
| SUBCKT/subckt_a5.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.54; /20000: VDDDEV#branch 8.46 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a5_hs.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.54; /20000: VDDDEV#branch 8.46 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a0.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a1.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a1_dup.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a2.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a2_dup.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a3.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a3_hs.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a4.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| SUBCKT/subckt_a4_hs.cir | 100 ps (TSTOP/300000) | 300552 / 300130 | /2000: VIN1#branch 6.29; /20000: VDDDEV#branch 7.92 | pass, worst VDDDEV#branch 0.716 |
| HB/RC/simpleRC2.cir | 20 ns (TSTOP/500000) | 500018 / 500010 | /2000 and /20000: V(2) 27.96 (Xyce's own step is finer than both, so its run does not change) | pass, worst N3 0.008 |
| SANDLER23/sandler23.cir | 20 ns ((TSTOP-TSTART)/100000) | 100321 / 100293 | /2000: X1:12 2.87; /20000: X1:12 1.61 | pass, worst X1:11 0.973 |

Both simulators' values where the listed mismatch was:

- subckt_a5 and subckt_a5_hs, v(out) at 16.55 us. Xyce: 4.80602 (as written), 4.80602 (/2000),
  4.80605 (/20000), 4.80605 (100 ps). ngspice: 4.81314, 4.79805, 4.80641, 4.80605.
- simpleRC2, v(2) at 8.84501 ms. Xyce: 1.38454e-3 (as written and at /20000), 6.45044e-4 (20 ns).
  ngspice: 6.44816e-4 (as written), 6.45039e-4 (20 ns). It is Xyce that moves: its own step error,
  as in HB/RC/simpleRC.cir (simplerc-closed-form.md).
- SANDLER23, X1:12 at 1.20014 ms. Xyce: -0.180707, -0.179263 (/2000), -0.180756 (/20000),
  -0.181392 (/100000). ngspice: -0.179488, -0.179488, -0.17829, -0.181502.

SANDLER23 passes by 3% of the tolerance, and neither simulator's own last two levels agree yet
(Xyce /100000 against /20000: X1:12 1.59; ngspice: 3.48): the certificate is the pair at this
step, not a limit. subckt_a5: Xyce at 100 ps against its /20000 passes (0.71); ngspice does not
(9.26), it is ngspice that moves.

The nine `subckt_a0`…`a4` rows are `subckt_a5`'s inverter chain with the
subcircuit written another way: one set of numbers for all nine, written
decks of the tool-batch record (S4, e2ad5d7), run 2026-11-11 (ZERO-LEDGER
P1-066).

Source: ZERO-LEDGER P1-045 (the tool and the logs are in the row's spec evidence).
