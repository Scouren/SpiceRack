# S-GSTEP-FLOATING-EMITTER — third-simulator and closed-form

One PNP with its emitter to ground through 1 TOhm (GMIN_STEPPING, HOMOTOPY,
BUG_920, BUG_966, Output/HOMOTOPY, BJT_OFF, DIAGNOSTIC DC_GStep).

- Closed form: the junctions can source at most IS+ISE+ISC ~ 5e-14 A in
  reverse, so |V(5)| <= 1e12 * 5e-14 = 0.05 V.
- Xyce 7.10: V(5) = -12.4675 V (emitter current 12.5 pA: its gmin, not the device).
- ngspice-45: -3.9e-4 V at gmin 1e-12; -6.638e-4 V for any gmin <= 1e-15.
- LTspice 26.1.1, the same op: V(5) = -3.8987e-4 V, ngspice's at gmin 1e-12.

Source: research/mismatches.md §2, §7.
