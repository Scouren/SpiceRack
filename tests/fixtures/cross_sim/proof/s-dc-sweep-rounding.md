# S-DC-SWEEP-ROUNDING — source (ngspice's sweep arithmetic)

TERNARY_OP: ngspice-45 adds the step at each point
(`spicelib/analysis/dctrcurv.c:470`, `VSRCdcValue += TRCVvStep`), so V(1) is
2.16e-15 where Xyce 7.10 has 0; every `V(1)>0` and `1/V(1)` output flips at
that point (4.6e14). A discontinuity at a threshold the sweep straddles.

Source: research/mismatches.md §2 (S-DC-SWEEP-ROUNDING).
