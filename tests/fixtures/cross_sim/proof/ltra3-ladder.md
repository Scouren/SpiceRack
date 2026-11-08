# S-LTRA3-LADDER — LTRA/ltra3: third-simulator (Xyce 7.10 is the outlier)

The third oracle is the line itself: the LTRA's per-unit-length R, L, G, C
as a 1096-section and a 2191-section RLGC ladder, run on ngspice-45.

Against the ladder, ngspice passes every top-level node; Xyce fails node 3
(0.0327 against a tolerance of 0.0144). Raw files read at TSTOP/10000.

Source: research/settle-ngspice.md, TRAN-PERSIST LTRA/ltra3;
ZERO-LEDGER P1-045.
