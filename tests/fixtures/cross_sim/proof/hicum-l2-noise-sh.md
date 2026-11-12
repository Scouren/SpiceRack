# HICUM-L2-NOISE-SH — reference-data (the CMC `.sp.reference` beside each deck)

The CMC HICUM/L2 noise decks, `cmcqa/hicum/hicum_noise_npn_<card>_ac_<bias>.sp`
(13 model cards, five biases V(base) = 0.7 … 1.1 V at V(coll) = 1 V, 16.85 °C):
`hn 0 n_coll vcoll_0 1` and `.noise v(n_coll) vin dec 2 1000 1e+011`, so the
output noise is the collector current noise through 1 Ω. The reference table
is `Freq N(coll)` in V²/Hz: the output noise density is sqrt(N(coll)).

Six cards turn self-heating on (`flsh=1`, `rth=1113.4`): `cornoise`, `full_sh`,
`full_subcoupl`, `full_subcoupl_peri_bias`, `full_subcoupl_peri_con`,
`full_subtran`. At their biases 2, 3 and 4 (V(base) = 0.9, 1.0, 1.1 V) the
output noise at 1 kHz, in V/√Hz:

| bias | reference | ngspice-45 (hicum2) | Xyce 7.10.0 (level 234) |
|---|---|---|---|
| 2 | 1.18130e-10 | 1.15231e-10 (2.45 % under) | 1.18097e-10 (0.028 % under) |
| 3 | 1.62555e-10 | 1.52813e-10 (5.99 % under) | 1.62576e-10 (0.013 % over) |
| 4 | 1.61766e-10 | 1.56133e-10 (3.48 % under) | 1.61756e-10 (0.006 % under) |

Over all 17 frequencies of those 18 decks Xyce stays within 0.042 % of the
reference; ngspice is up to 2.45 % (bias 2), 5.99 % (bias 3; `cornoise`
19.6 %) and 4.27 % (bias 4; `cornoise` 21.8 %) from it. ngspice-45's hicum2
is the outlier against the deck's own reference.

The other 47 decks of the family (the seven cards with `flsh=0`, and biases 0
and 1 of the six above, where little power is dissipated) pass the comparer,
both simulators within 0.11 % of the reference at every frequency: the
difference is ngspice's noise at the raised device temperature.

The input-referred vector (`inoise_spectrum`, the list lines' first number) is
1e10 times the output's on both simulators (`vin` drives an isolated 1 Ω), so
it carries the same ratio.

Run 2026-11-11, the source deck on ngspice-45 and the deck written for Xyce on
Xyce 7.10.0, all 65 decks. Source: ZERO-LEDGER P1-066.
