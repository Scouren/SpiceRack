# S-MOS6-XYCE-LAMBDA — third-simulator (Xyce 7.10 is the outlier)

The MOS6 inverters `MOS6/invert_mos6`, `BUG_1041_SON/mos6_invert_scaleRef`,
`mos6_invert_scale2Ref`, `ISSUE_239/mos6_test2Ref` (xyce-ngspice), each
with `lambda=0.02`.

At TMAX 0.1 ns, vout min / max / steady: ngspice-45 −0.2721 / 4.7251 /
4.43076; VACASK `spice/mos6.osdi` −0.2729 / 4.7250 / 4.43076; Xyce 7.10
−0.326 / 4.4255 (no rising overshoot) / 4.4255. (Caveat: the OSDI module
shares SPICE3 lineage with ngspice.)

Cause, in Xyce: `N_DEV_MOSFET6.C:3055` reads `if(lambda0Given)` where
ngspice's `mos6/mos6set.c:93-97` reads `if(!…lamda0Given)`, so Xyce drops
LAMBDA when LAMBDA0 is absent. Vd 5, Vg 3, W/L 175/5: VTO only, both
2.31175e-3 A; with lambda=0.02, Xyce 2.31175e-3, ngspice 2.54293e-3
(= 2.31175e-3 · 1.1); Xyce given lambda and lambda0, 2.54292501e-3. Every
other parameter bisected matches to 6 digits. The writer workaround is
P3-073; until it lands these decks differ by Xyce's reading of LAMBDA.

Source: research/settle-ngspice.md (TRAN-PERSIST MOS6),
research/settle-xyce.md (MOS6); ZERO-LEDGER P1-045.
