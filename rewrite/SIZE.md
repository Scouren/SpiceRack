# SIZE: the new pipeline against the old, in lines

Measured, not judged. Rerun at each milestone from the repository root:

```text
python3 rewrite/size/size.py <rev>
```

It reads git objects at `<rev>` (no checkout) and rewrites `rewrite/size/used.txt` and
`rewrite/size/dead.txt` (the old files the new pipeline still uses, and the rest). Its docstring
states the definitions: test files (a name holding "test") are out of every row; **code lines**
are `.github/scripts/check_style.py`'s (not blank, not starting with `//`, before a file's
`#[cfg(test)]` item); **used** is a reachability closure over module references from
`src/dialect` and `src/passes` (the facade's old-route switch `src/dialect/facade.rs` and test
files apart), with its limits listed.

## At 56835ee3 (`ir-spine`)

| area | files | lines | code lines |
|---|---|---|---|
| NEW crates/ir/src | 89 | 33,062 | 24,450 |
| NEW src/dialect | 92 | 28,847 | 22,053 |
| NEW src/passes | 75 | 9,695 | 6,773 |
| NEW total | 256 | 71,604 | 53,276 |
| OLD src/syntax | 165 | 48,418 | 32,947 |
| OLD src/syntax, used by the new pipeline | 33 | 9,535 | 5,677 |
| OLD src/syntax, dead once the facade routes through the new pipeline | 132 | 38,883 | 27,270 |

## Languages each side reads and writes

| side | reads | writes | source |
|---|---|---|---|
| NEW | ngspice, VACASK, IR text | ngspice, VACASK, IR text | `src/dialect/mod.rs` `BUILTIN` (rows `ir`, `ngspice`, `vacask`, each with `read` and `write`) |
| OLD | ngspice, HSPICE, Xyce, LTspice, Spectre, VACASK; reader-only: ngspice-given (the Python decks), spicerack | ngspice, ESPice, HSPICE, Xyce, LTspice, Spectre, VACASK; writer-only: CDL, Verilog | `src/syntax/lang.rs` `Lang::*`, `src/syntax/dialects/*.rs`; the reader/writer split as `rewrite/INV-constructs.md` states it (dialect keys) |

## Not in the table

Test code (whole `.rs` files, at 56835ee3): `crates/ir/tests` 14,288 lines; the new pipeline's
test binaries in `tests/` (`dialect_*`, `passes_*`, `pass_laws`, `layering`, `test_spine`,
`test_pair`, `common/known_*`) 16,418; every other `tests/*.rs` 23,792. Fixtures are not
counted. Other `src/` files that call into `src/syntax` (old-pipeline consumers a facade switch
would reroute; not judged): `import.rs`, `lint.rs`, `measure_parse.rs`, `normalize.rs`,
`result.rs`, `simulation.rs`, `veriloga.rs`, `lib.rs`.
