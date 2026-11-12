# S-HSA-HSPICE-CARD — simulator-fails (ngspice-45's hsa mode reads no HSPICE analysis card)

The hsa cell runs the written HSPICE deck on `ngspice -D ngbehavior=hsa`, the harness's stand-in
for HSPICE. That mode changes how ngspice-45 reads parameters and quoting; it keeps ngspice's own
analysis cards. Three cards the HSPICE manual allows, each as the writer spells it, stop it:

| card as written | HSPICE manual | ngspice-45 hsa | ESPice 0.3.0 HSPICE reader (`espice-hspice --tokenizer=hspice`) |
|---|---|---|---|
| `.ac dec 2 1k 100k` + `.noise V(out) vin 0` | Star-Hspice Manual 1998.2 ch.9 p.9-16: `.NOISE ovv srcnam inter`, the frequencies those of the `.AC` card | "Error on line 8 or its substitute: .noise v(out) vin 0", exit 1, no raw file | runs: an AC plot (5 points) and a noise spectral density plot |
| `.pz v(out) Vin` | ch.25 p.25-3: `.PZ ov srcnam` | "Error on line 7 or its substitute: .pz v(out) vin", exit 1, no raw file | runs: a pole-zero plot |
| `.tran 10n 2u START=1u` | ch.7 p.7-4: `.TRAN tincr1 tstop1 … <START=val>` | "Error on line 7 or its substitute: .tran 10n 2u start=1u / Error: unknown parameter on .tran - ignored", exit 1, no raw file | runs: a transient plot (111 points) |

Runs, 2026-11-11: ngspice-45 and the `espice-hspice` build `flake.nix` pins, on the HSPICE text
SpiceRack (p7-integrate 001c5b7) writes for three RC decks
(research/impl/evidence/P1-066-hsa-hspice-cards/: `noise.sp`, `pz.sp`, `tran.sp`, each `w_*/*.hspice.sp`,
`run.sh`, `results.txt`). ngspice's own cards for the three are `.noise v(out) vin dec 2 1k 100k`,
`.pz in 0 out 0 vol pz` and `.tran 10n 2u 1u` (ngspice-45 manual §11.3.4, §11.3.6, §11.3.10): plain
ngspice runs each original, so the harness's control does not show the failure.

The 80 decks of `ngspice-hspice-hsa.txt` that cite this file: 74 with `.noise` (the 65 CMC
`hicum_noise_*` decks and 9 ngspice examples), 4 with `.pz`, 2 with `START=`. On the ESPice HSPICE
cell the same written decks: 42 pass against ngspice on the original, 36 are
`simulator-differs:source` (ESPice differs from ngspice on the original too), 1 is an open
`mismatch` (`ngspice/examples/noise/bipolar-noise.sp`), 1 `simulator-fails`.

Source: research/impl/P1-066-hsa-hspice-cards.md; ZERO-LEDGER P1-066.
