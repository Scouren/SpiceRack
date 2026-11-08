# XYCE-GLOBAL-FLOATING-NODE — self-inconsistency (Xyce's own spike)

Xyce_Regression CONNECTIVITY/connect.cir (and BUG_318 bug_318, bug_318_noscope,
SANDLER28/sandler28, the same opamp): each UA741T instance has `Cbad 3
$G_BAD_PATH 1pf`, a 1 pF capacitor from the 15 V rail to Xyce's global node
`$G_BAD_PATH` (Xyce RG §2.3.4), which stream B writes for ngspice as
`.global G_BAD_PATH` (e9c1a25). The node floats: the four capacitors carry no
current and change no node voltage. Run 2026-11-08 by hand, V(10):

| t | Xyce 7.10, `$G_BAD_PATH` (global) | Xyce 7.10, node local | ngspice-45 global | ngspice-45 local |
|---|---|---|---|---|
| 1.0000 ms | -3.71376e-4 | -3.71384e-4 | -3.71376e-4 | -3.71376e-4 |
| 1.0191 ms | -2.88200e-4 | -3.72395e-4 | -3.71755e-4 | -3.71755e-4 |
| 1.0500 ms | -3.71628e-4 | -3.71553e-4 | -3.71567e-4 | -3.71567e-4 |
| 3.0000 ms | -3.71394e-4 | -3.71357e-4 | -3.71376e-4 | -3.71376e-4 |

Xyce alone answers differently for the two physically identical circuits (a
8.4e-5 V spike at the 1 ms edge with the node global); ngspice gives one
answer either way, and it is Xyce's local-node one. The written deck is the
circuit.

Source: ZERO-LEDGER P1-054.
