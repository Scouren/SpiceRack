# S-EKV26-TWO-CAMPS — third-simulator (two pairs, up to ×2.3 apart)

EKV 2.6, Xyce's `Netlists/EKV26/test_ekv26_nmos.cir` card. Xyce 7.10 level
260 equals the EPFL Verilog-A (OpenVAF `integration_tests/EKV/ekv.va`, rev
15, compiled by OpenVAF and run in ngspice-45 through OSDI) within 1e-3;
Spectre 25.1 `ekv update=2.6` equals HSPICE 2026.03 level 55 within 1.2e-3
to 4.5e-3; the two pairs differ by up to ×2.3. At vg = 2, vd = 0.1: Verilog-A
5.347e-5, Xyce 5.347e-5, Spectre 8.874e-5, HSPICE 8.875e-5. No pair is
shown wrong, so Xyce level 260 ↔ Spectre `ekv` is refused (W0134) in both
directions with this proof.

Pins: `tests/spectre/recorded/ekv.pin`, `tests/hspice/recorded/ekv.pin`
(iccad4, 2026-11-07). Source: research/settle-remote.md, "Third oracle for
the model families whose numbers differ"; ZERO-LEDGER P1-052, P4-098,
P4-103.
