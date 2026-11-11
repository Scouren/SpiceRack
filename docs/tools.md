# Tools and the Rust crate

## spice2rack: convert a deck to every dialect

`spice2rack` reads existing SPICE decks and writes them out for every backend.

```bash
cargo run --no-default-features --bin spice2rack -- -o out/ deck.sp [more.sp ...]
```

For `deck.sp` it writes `deck.ngspice.sp`, `deck.hspice.sp`,
`deck.ltspice.cir`, `deck.spectre.scs`, `deck.vacask.sim` and
`deck.espice.sp`. A dialect that cannot express the circuit is reported with
its diagnostic codes and skipped; the rest are still written, and what each
kept or left out is printed beside it.

| Flag | Effect |
| --- | --- |
| `--check` | check each deck is a fixpoint: written in its own dialect and read back, it writes the same text |
| `--dialect hspice` | read HSPICE decks (default: ngspice; also ltspice, xyce, spectre, vacask) |
| `--to espice` | write that dialect alone (`ngspice`, `hspice`, `ltspice`, `spectre`, `vacask`, `espice`, `xyce`, `cdl` or `verilog`) and print the written deck's absolute path on stdout; a deck the dialect refuses is not written and the exit code is 1 |
| `--kit` | with `--to`: also write every file the deck reaches through `.include` and `.lib`, all or none |

### One dialect, and a deck with its model files

```bash
spice2rack --dialect spectre --to espice --kit -o out/ tb/top.scs
```

writes `out/top.espice.sp` and prints that path, alone, on stdout (one line a
deck). Notes and refusals go to stderr with their codes. The exit code is 0,
1 when a deck was refused or could not be read, and 2 on a usage error.

With `--kit` the files the deck reaches are written too. A file under the
deck's directory keeps its path from there (`tb/models/nch.scs` is written to
`out/models/nch.scs`); a file anywhere else (a PDK's absolute path,
`../shared/x.lib`) goes by its whole path under `out/_root/`. Every
`.include` and `.lib` path in the written files is the absolute path of the
written copy, so the deck runs from any working directory and never picks up
an unconverted file of the same name. A Verilog-A source is left where it is
and named by its absolute path. If a file is refused or missing, or a copy
would land on a file the deck reads, nothing is written and the exit code
is 1. A file the deck names in another way (a PWL `file=`) is not part of the
kit: its path is written as it was.

The converter builds on its own, without Python or a simulator:

```bash
nix build github:OpenFAT/SpiceRack#spice2rack   # result/bin/spice2rack
```

A PDK model file converts the same way. Its library sections (SPICE
`.lib tt` ... `.endl tt`, Spectre and VACASK `section tt` ... `endsection`,
in Spectre bare or inside a `library`) are read with what each holds (its
`.param`s, models, `.subckt`s, `.include`s and `.lib` calls), and
are written back as sections in every dialect that has them: ngspice, HSPICE,
Xyce and ESPice as `.lib`/`.endl`, Spectre and VACASK as `section` (inside
its `library` when it had one). SPICE bins (`nch.1`, `nch.2`) become a Spectre model group and
back. LTspice reads `.lib tt` as a file, so it has no section form and is
skipped.

Monte Carlo statistics convert too.
SPICE's `agauss`, `gauss`, `aunif`, `unif` and `limit` reach Spectre as
`rack_*` functions of normalized variables varied in a `statistics` block
(`process` at a file's or section's top, `mismatch` inside a subcircuit), and
read back as the calls; VACASK writes them as they are (`limit` spelled out).
A Spectre `statistics` block is read and written to SPICE as
`agauss`/`aunif` draws. LTspice has none of the functions and refuses them.

## Using the Rust crate

```toml
[dependencies]
spicerack = { git = "https://github.com/OpenFAT/SpiceRack", default-features = false, features = ["backend"] }
```

| Feature | Adds |
| --- | --- |
| (none) | the transpiler (`syntax`: read any dialect, write any other, coded diagnostics) and the `.raw` readers; no I/O, builds for `wasm32-unknown-unknown` |
| `backend` | running simulators, Verilog tools, backend checks |
| `python` (default) | the Python bindings; implies `backend` |

`spicerack::syntax`'s module notes show how to load a kit, write a file in
another dialect and read its diagnostics.
