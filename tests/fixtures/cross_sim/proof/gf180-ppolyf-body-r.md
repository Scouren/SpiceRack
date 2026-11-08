# S-GF180-PPOLYF-FIRST-BRACE — source (ngspice-45 misreads the source)

The poly resistor bodies of gf180's `sm141064.ngspice`, `ppolyf_u` shown.

gf180's `ppolyf_u` body resistor is `rb 11 21 ppolyf_u_body l=r_l w=r_w
+r='(r_temp*r_n*(r_rsh0+…abs(v(11,21))…))*(…)'` (`sm141064.ngspice`).
ngspice-45 rewrites an R whose value holds `v(…)` into a B source and takes
the first `{` on the line as the value (`frontend/inpcom.c`,
`strchr(cut_line,'{')`): here that is `l=r_l`, so the body runs as R = its
length. ESPice on the same source deck reads `r=`.

`cmcqa/gf180/gf180_res_RES07a_ff_T25.sp` (op), 2026-11-08, ngspice-45 and
ESPice 0.3.0 from `nix develop .#testing`:

| run | v(xrn_0.11) | v(xrn_5.11) | v(xrn_45.11) |
|---|---|---|---|
| ngspice-45, the source | -1.64942 | -1.64999 | -1.64999 |
| ngspice-45, `r='…'` moved before `l=` (the value first) | -3.29530 | -3.29530 | -2.95140 |
| ESPice 0.3.0, the source | -3.2952952 | -3.2952991 | -2.9513977 |

`cmcqa/gf180/gf180_res_RES06a_typical_T25.sp` (`npolyf_u`), the same runs:

| run | v(xrn_0.11) | v(xrn_5.11) |
|---|---|---|
| ngspice-45, the source | 1.649125 | 1.649987 |
| ngspice-45, the value first | 3.297163 | 3.297166 |
| ESPice 0.3.0, the source | 3.2971626 | 3.2971661 |

With the value first, ngspice agrees with ESPice to its printed digits: the
difference is ngspice's reading of the source, not the translation. Every
gf180 poly resistor body (`npolyf_s/u`, `ppolyf_s/u`, `ppolyf_u_1k/2k/3k`,
9 subcircuits) is written this way, and every listed deck is a
`gf180_res_*` deck instantiating one.

Source: research/mismatches.md §3 (gf180 ppolyf); ZERO-LEDGER P1-035.
