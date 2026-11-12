# S-NGSPICE-OUTLIER-3RD — third-simulator (ngspice-45 is the outlier, three agree)

Runs, 2026-11-11: ngspice-45, Xyce 7.10.0, VACASK (`nix develop .#testing`), ESPice 0.3.0. ESPice
reads the ngspice text itself (the original, or for the Xyce-corpus deck the ngspice deck written
from it); Xyce and VACASK run the deck SpiceRack (p7-integrate 001c5b7) writes for them.

`gm6` (`circuitsim90/MOS2/gm6.sp`, the Xyce corpus's `CircuitSim90/MOS2/gm6.cir` is the same
circuit): v(13), the node between `mn3` and `mn4`, both off at the operating point (gate v(5) = 0,
MOS level 2 with no NFS), at 125 °C:

| simulator | v(13) at the operating point |
|---|---|
| ngspice-45 | 0.40898654762563585 V |
| Xyce 7.10.0 (first transient point) | -8.3308381649482e-23 V |
| VACASK | 0.0 V |
| ESPice 0.3.0 | -1.0214312542713579e-16 V |

`vreg` (`circuitsim90/BJT/vreg.sp`, v(2); the Xyce corpus's `CircuitSim90/BJT/vreg.cir`, V(1)), the
`.dc vcc 0 15 1` sweep at VCC = 2 V, the regulator starting up; from 4 V on all agree to 1e-6:

| simulator | vreg.sp v(2) at 2 V | vreg.cir V(1) at 1 / 2 / 3 V |
|---|---|---|
| ngspice-45 | 0.5586759973994992 | 0.0122306744 / 0.556961605 / 0.71517444 |
| Xyce 7.10.0 | 0.5652145450319832 | - / 0.5630442895063699 / - |
| VACASK | 0.5651719649161091 | not run |
| ESPice 0.3.0 | 0.5651766095155096 | 0.0129016616 / 0.563010518 / 0.715371105 |

Three of four agree within 8e-5 on `vreg.sp` and two of three within 6e-5 on `vreg.cir`; ngspice-45
is 1.1-1.2 % under them (tolerance 2 * (RELTOL * swing + VNTOL) = 4.6e-3 and 1.5e-3). No writer
change: the difference is ngspice-45's against the same circuit.

Source: research/impl/P1-066-ngspice-outlier-3rd.md and its evidence
(research/impl/evidence/P1-066-ngspice-outlier-3rd/); ZERO-LEDGER P1-066.
