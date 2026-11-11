# S-BSIM1-NGSPICE-L4 — third-simulator (ngspice-45 level 4 is the outlier)

BSIM1, the ngspice `tests/bsim1` card, Id over vgs at vds 0.05, 2.5 and 5 V.
Spectre 25.1 `bsim1` and HSPICE 2026.03 level 13 agree to 3.1e-6 (maximum
relative difference); ngspice-45 level 4 differs from both by 7.7 %, 4.5 %
and 3.6 % at the three vds. At vg = 3, vd = 2.5: ngspice 5.1514e-4, Spectre
5.2746e-4, HSPICE 5.2747e-4. Two of three agree; ngspice's BSIM1 is the
outlier, so the name-and-unit map of the card (vfb→vfb0, phi→phi0,
eta→eta0, u0→u00, u1→u10, dl→dl0, dw→dw0; tox µm→m) is right and a
ngspice↔Spectre/HSPICE BSIM1 difference of this size is the simulators'.

Pins: `tests/spectre/recorded/bsim1.pin`, `tests/hspice/recorded/bsim1.pin`
(iccad4, 2026-11-07). Source: research/settle-remote.md, "Third oracle for
the model families whose numbers differ"; ZERO-LEDGER P1-052, P4-025.
