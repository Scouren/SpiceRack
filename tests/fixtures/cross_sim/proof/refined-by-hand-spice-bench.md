# S-NUMERIC-CONVERGES by hand, the spice-bench cells: refinement certificates

Decks of the ngspice corpus the harness's own refinement leaves a `mismatch` and a finer step
settles: the harness refines to TMAX (TSTOP-TSTART)/2000 and /20000, a deck's own `.tran` card
only, and no VACASK deck (`sim.rs::refined`). Each row: the ngspice original on ngspice-45 and the
deck SpiceRack writes for the target (spec-s4 e2ad5d7) on Xyce 7.10.0 or VACASK (unstable-2026),
both at the TMAX named (VACASK: `maxstep=`), every shared vector of the transient compared as
`tests/test_cross_sim/cmp.rs` compares it (1% of the swing plus twice the floor, through the 1%
time tube). "worst" is the largest distance from the band in units of the tolerance: under 1
passes. Run 2026-11-11.

| deck | target | TMAX | points ngspice / target | at the harness's levels | at TMAX |
|---|---|---|---|---|---|
| ngspice/examples/snapshot/adder_mos_circ.sp | Xyce | 10 ps (TSTOP/100000) | 100281 / 100275 | as written: v(x1.x1.x1.x6.net.1) 58.2; /2000: the same vector 54.1; /20000: i(vin3b) 8.01 | pass, worst i(vcc) 0.699 |
| ngspice/examples/snapshot/adder_mos_circ.sp | VACASK | 10 ps (TSTOP/100000) | 100281 / 100424 | as written: v(x1.x1.x1.x6.net.1) 65.1; /2000: the same vector 56.2; /20000: i(vin3b) 7.91 (by hand: the harness refines no VACASK deck) | pass, worst i(vin2a) 0.806 |
| ngspice/examples/snapshot/adder_mos.sp | Xyce | 10 ps (TSTOP/100000) | 100281 / 100275 | none reaches it: its `.tran` is in the file it includes; as written v(10) 1.45 | pass |
| ngspice/examples/snapshot/adder_mos.sp | VACASK | 10 ps (TSTOP/100000) | 100281 / 100424 | none reaches it (the same); as written v(10) 2.52 | pass |

`adder_mos.sp` is `.include adder_mos_circ.cir` and `.save vcc#branch 10`; the included file is
`adder_mos_circ.sp` byte for byte but for the case of `.END`. Its rows are the same two runs with
the `.tran` card of the included file refined (the source's, and the written copy's), compared on
the two vectors it saves; the full comparison of that circuit is the `adder_mos_circ.sp` row.

The circuit is a 4-bit adder of 36 NAND gates (BSIM3, every parameter default) driven by 10 ns
edges, `.tran 1ns 1000ns`. As written ngspice takes 1541 points, Xyce 5958, VACASK 5746. The
listed vectors are peaks of capacitively coupled nodes; all three simulators move with the step
and meet:

| vector, time | level | ngspice | Xyce | VACASK |
|---|---|---|---|---|
| v(x1.x1.x1.x1.net.1), 606.5 ns | as written | 1.69181 | 1.60376 | 1.61331 |
| | /2000 | 1.73070 | 1.73376 | 1.76012 |
| | /20000 | 1.79757 | 1.79744 | 1.79704 |
| | /100000 | 1.79821 | 1.79808 | 1.79764 |
| v(10), 240.26 ns | as written | -0.34212 | -0.28420 | -0.17633 |
| | /2000 | -0.28021 | -0.27817 | -0.16793 |
| | /20000 | -0.28382 | -0.28367 | -0.28368 |
| | /100000 | -0.28495 | -0.28492 | -0.28500 |
| i(vin4b), 661.425 ns | as written | -6.354e-6 | -8.677e-6 | -8.665e-6 |
| | /20000 | -9.421e-6 | -9.020e-6 | -8.783e-6 |
| | /100000 | -9.057e-6 | -9.048e-6 | -9.036e-6 |

Source: ZERO-LEDGER P1-066 (the tool and the logs: research/impl/evidence/P1-066-adder-refinement/).
