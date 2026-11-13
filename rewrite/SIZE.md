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

## The IR's breadth ahead of its dialects

What `crates/ir/src` holds that neither ngspice nor VACASK reads or writes today: the 217 features
`unspoken` on both sides of the L7 list (`tests/fixtures/dialect/l7_missing.txt` on `ir-next`
43a034a4, copied to `rewrite/size/unspoken.txt`). Counted on `ir-rewrite` 1fbf7eb2's crate, with
the same code-line definition as above:

```text
python3 rewrite/size/breadth.py rewrite/size/unspoken.txt
```

**Row level** (a whole row type no reader of the two builds). *Whole blocks*: the items headed
by the group's own types (`struct`, `enum`, `impl … for`, its field-table entry), which would be
deleted with it. *Other lines*: code lines elsewhere naming one of its identifiers as a word
(match arms, `ALL` lists, generator arms, dispatch rows), an upper bound because a line may also
name something else. Walkers, dispatch macros and the field-table machinery are attributed to no
group. "Unspoken" means no reader of the two builds it today, not that neither simulator has it:
VACASK has `hb`, `pac`, `pnoise` and module instances (inventory G19, B24).

| group | planned dialect (inventory) | code lines, whole blocks | other code lines naming it (upper bound) |
|---|---|---|---|
| Analysis Hb (harmonic balance) | xy hs es sp va (INV G16, G19) | 73 | 25 |
| Analyses Pac, Pnoise, Pxf, Pstb (periodic small-signal) | sp (G18), va: pac pnoise (G19) | 182 | 38 |
| Analysis Net (two-port network) | lt hs (G37, H15) | 41 | 13 |
| Analysis TimedOp | hs sp (G42) | 20 | 9 |
| Item Statistics (variations) | sp (G17); va mc | 281 | 42 |
| Elements Laplace, FreqTable, Delay (and their detail rows) | Laplace: lt hs es, ng via XSPICE (D23); Delay: hs, xy, lt (D90); FreqTable: hs (D106) | 62 | 153 |
| Element FluxInductor | lt (D80) | 0 | 11 |
| Element Igbt | lt (D103, F22) | 0 | 12 |
| Element Module (Verilog-A module instance) | sp, ng `N` card, va (B24) | 0 | 34 |
| Wave Pattern | hs xy (E32) | 15 | 29 |
| **total** | | **674** | **366** |

**Finer level** (a member that is a field, a flag, a family, an option key or an operator of a row
the readers do build): nothing is whole, so only the code lines naming a member qualified
(`::Name`) count, an upper bound. Rows overlap the row-level table (`Analysis`, `Elem`, `Item`,
`Wave`) and each other (`Func`/`FuncDyn`, `Binary`/`BinaryDyn` name the same members), so they are
not summed. Members with generic names (`DrawPer(Run)`, `DrawPer(Instance)`, `Expr(List)`,
`Elem(Module)`, `Elem(Delay)`) are overcounted by unrelated `::Run`, `::List`, `::Module`, `::Delay`.
Exact attribution would need, per member, its field-table entry, its `features` flag line, its
`check` arm and its generator arm read by hand: about 170 members.

| member group (finer level) | members | code lines naming one, qualified `::Name` (upper bound) |
|---|---|---|
| Analysis | 7 | 22 |
| Binary | 1 | 8 |
| BinaryDyn | 1 | 8 |
| Circuit | 41 | 131 |
| DistDyn | 1 | 4 |
| DrawPer | 2 | 66 |
| Elem | 6 | 129 |
| Expr | 1 | 48 |
| Extra | 5 | 16 |
| Family | 30 | 112 |
| Func | 4 | 22 |
| FuncDyn | 3 | 16 |
| Item | 1 | 8 |
| Option | 4 | 12 |
| Placed | 58 | no code of its own: a (scope kind, item kind) pair the generic feature walk derives |
| Run | 51 | 144 |
| Wave | 1 | 12 |


The planned dialect of each finer member is in `rewrite/INV-constructs.md` by its row; not
transcribed here.
