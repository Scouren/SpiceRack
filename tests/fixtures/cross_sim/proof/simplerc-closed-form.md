# SIMPLERC-XYCE-STEP — closed-form (Xyce's own step error)

Xyce_Regression HB/RC/simpleRC.cir and MPDE/RC/simpleRC.cir: `v1 1 0 sin 0 1V
1e5 0 0`, `r1 1 2 1k`, `c1 2 0 2u`, `.tran 0 1.0e-2`, `.options timeint
reltol=1e-3`. Closed form of the RC low-pass from rest (tau = 2 ms, w = 2 pi
1e5): v(2) = A sin(w t + ph) + A sin(-ph) exp(-t/tau), A = 1/sqrt(1 + (w
tau)^2), ph = -atan(w tau). Run 2026-11-08 by the cross-sim harness (kept
runs): v(1) is the exact sine on both to 9e-13. v(2): Xyce 7.10 off the
closed form by 3.83e-5 V at its own points (127,302 points, its step control
at reltol 1e-3), ngspice-45 on the written deck (`.tran 200n 10m`, 50,011
points) by 2.07e-6 V. The swing of v(2) is 1.59e-3 V, the tube 1.79e-5 V:
the listed mismatch is Xyce's step error, which ngspice does not share. The
written deck is the circuit (team-lead's P1-053 measurement of v(1) agrees).

Source: ZERO-LEDGER P1-053.
