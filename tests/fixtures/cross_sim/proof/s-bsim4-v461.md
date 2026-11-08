# S-BSIM4-V461 — self-inconsistency (version)

The Berkeley BSIM4 test cards say `VERSION = 4.6.1`. Xyce 7.10 runs its own
4.6.1 code; ngspice-45 selects BSIM4v6 (4.6.5) for "4.6" (`inpdomod.c:297-310`).
`BSIM4/test1.X` I(vds), max relative difference: Xyce 4.6.1 vs ngspice 6.0e-3;
Xyce 4.7 vs ngspice 6.7e-4 (inside 2*RELTOL); ngspice's BSIM4v6, v7 and 4.8.2
give identical numbers on this card; Xyce's own 4.6.1 and 4.7 differ by 0.6 %,
its 4.8.2 by 46 %. Xyce's 4.6.1 is the outlier among five implementations.
ACtests gstage v(3) at 100 Hz: Xyce 4.6.1 -6.6044e-4, Xyce 4.7 -6.63882e-4,
ngspice -6.64317e-4 (settle-ngspice U-AC-MOS). The writer is faithful.

Source: research/mismatches.md §2 (S-BSIM4-V461); settle-ngspice.md row 85.
