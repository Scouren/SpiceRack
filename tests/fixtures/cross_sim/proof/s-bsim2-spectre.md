# S-BSIM2-SPECTRE — third-simulator (Spectre 25.1 bsim2 is the outlier)

BSIM2, the ngspice `tests/bsim2/test.cir` card (L=10u W=50u), vg 0…5 V at
vds 0.05, 2.5 and 5 V, 27 °C. HSPICE 2026.03 `level=39 spice3=1` on the card
verbatim agrees with ngspice-45 level 5 to 2.5e-4, 4.2e-5 and 4.2e-5 at the
three vds; Spectre 25.1 `bsim2` differs from ngspice by 7.3e-2, 0.38 and
1.05 (up to 4× near threshold at vds = 5). At vg = 3, vds = 2.5: ngspice
-1.1356e-3, HSPICE -1.1357e-3, Spectre -1.2070e-3. Two of three agree;
Spectre's bsim2 is not Berkeley BSIM2, so ngspice level 5 → Spectre bsim2
(and back) is refused, and HSPICE takes the level-5 card as
`level=39 spice3=1`.

Pins: `tests/spectre/recorded/bsim2.pin`, `tests/hspice/recorded/bsim2.pin`
(iccad4, 2026-11-07). Source: research/settle-remote.md, "BSIM2 third
oracle: HSPICE level 39, units corrected"; ZERO-LEDGER P1-052, P4-025.
