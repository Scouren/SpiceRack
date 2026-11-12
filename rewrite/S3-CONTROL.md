# S3-CONTROL: the ngspice-45 control script, command by command (sub-slice S3.4)

Owner: `ng-control`. Evidence: the ngspice-45 source (`src/frontend/commands.c` line numbers
below; `inp.c`, `control.c`, `variable.c`, `options.c`, `com_option.c`, `device.c`,
`spiceif.c`, `runcoms.c`, `runcoms2.c`, `dotcards.c`, `main.c`) and runs of the pinned
`ngspice` (`nix develop .#testing`), each a deck under
`tests/fixtures/dialect/ngspice/settle/` whose header states what the run printed. Codes are
tentative until the lead's answers (Step 0).

## 1. Settled facts

### Order (O)

| # | Fact | Evidence |
|---|---|---|
| O1 | Every `.control` block of the deck, every `*#` line outside one, wherever it stands (a false `.if` branch, a `.subckt` body, an included file), is one script in file order (includes inlined). A line starting `pre_` runs before the circuit is parsed, stripped of `pre_`. `+` continues a script line. | `inp.c:742-812`, `:847-859`; `order_two_blocks.sp`, `order_block_placement.sp`, `state_alterparam.sp` |
| O2 | The script runs after the circuit is parsed; the dot-card analyses do not run at their card. | `inp.c:1282-1301`; `order_two_blocks.sp` |
| O3 | `run` runs every dot-card analysis of the deck (by type rank) on the state of that moment; with no dot-card analysis it runs nothing. Each `run` makes a new plot per analysis. | `runcoms.c:403-411` `dosim("run")`; `order_run_three_cards.sp`, `order_run_no_card.sp` |
| O4 | `ngspice -b -r raw`: after the script every dot-card analysis runs once more, on the script's final state, and only that run reaches the raw file; the script's plots stay in memory. A deck whose analyses are all script commands writes no raw file. | `main.c:1547-1555`; `order_run_one_card.sp` (1 plot in the raw, 2 runs), `order_after_script.sp` (v(b) 0.25 after an `alter`), `order_script_only.sp` |
| O5 | `ngspice -b` (no `-r`): after the script the dot-card analyses run again only when the deck has `.plot`, `.print`, `.four`, `.meas`, `.op` or `.tf`. | `main.c:1556-1561`, `dotcards.c:93-161` `ft_savedotargs`; `order_run_one_card.sp`, `order_run_three_cards.sp` |
| O6 | SpiceRack's backend (`src/backend/ngspice.rs`) runs a deck whose script holds `run`, `op`, `ac`, `tran` or `dc` with `-b` and an appended write block (raw = every script plot, O5's later run not in it); any other deck with `-b -r` (O4). `noise`, `tf`, `pz`, `sens`, `disto`, `pss`, `sp` script commands are missing from its list, so their results are lost. | `ngspice.rs:19`, `:54-66`, `:128-156` |

### State (S)

| # | Fact | Evidence |
|---|---|---|
| S1 | `reset` rebuilds the circuit from the deck: it discards every `alter`, `altermod`, script `option`/`set` of a simulator option (temperature included) and script `save`; it keeps `.options`, `.temp`, `.save` cards and every `alterparam` given before it. | `runcoms2.c:172-240`; `state_reset.sp`, `state_option_temp.sp`, `state_set_temp_no_card.sp`, `state_save_reset.sp` |
| S2 | `alterparam p=v` edits the deck's `.param p` line (the rest of the line after `=` becomes `v`, one token; `.param` lines are split one per parameter first); nothing changes until the next `reset`, and it holds across every later `reset`. The last `alterparam` of a name before a `reset` wins. `alterparam sub p=v` edits the `.subckt sub` default and every `x … sub` line's value. | `inp.c:1735-1857`; `state_reset.sp` (0.3185 before reset), `state_alterparam.sp` |
| S3 | `option k=v` and `set k=v` are one command for a simulator option: both `cp_vset`, and `cp_usrset` passes a name the circuit knows to `if_option` (`US_SIMVAR`, stored in `ci_vars`, freed by `reset`). A script option beats `.options` and `.temp` until `reset`. | `com_option.c:104-131`, `options.c:444-450`, `variable.c:194-213`; `state_option_temp.sp` |
| S4 | `alter r1 v` (no parameter) sets the device's principal parameter (`parmlookup(…, NULL)`); `alter dev p v`, `alter dev p = v`, `alter @dev[p] = v`; `altermod mod p = v`, `altermod @mod[p] = v`. The value is the frontend's expression (`ft_getpnames_quotes`: vectors readable), or a `[ … ]` list. | `device.c:1249-1420`, `spiceif.c:935-962` |
| S5 | After `foreach v …` the altered target holds the last value (`LoopEnd::KeepLast` confirmed for `alter` and for `alterparam`+`reset`). | `subst.sp` (`show r1` = 0.5), `state_alterparam.sp` |

### Substitution (V)

| # | Fact | Evidence |
|---|---|---|
| V1 | A `foreach` variable is the word as written (`CP_STRING`): `1k`, `2.2k`, `0.5` substitute as those words, so `alter r1 $v` is exact. The word list is globbed, backquoted and substituted first (`{`, `` ` ``, `$`, `*`, `?`, `[`, `~` in a word are not constants). | `control.c:462-466`; `subst.sp` |
| V2 | A `set` number is stored as a double and reprinted with `%G`: `set x = 1.23456789k` gives `$x` = `1234.57`; a list element `1k` gives `1000`. | `variable.c:41-56`; `subst.sp` |
| V3 | `$&v` (a vector) prints six significant digits, `%G`: `1234.57`, `0.333333`, `1.23457E+11`, `1.5E-17` (INV 13.01's "six digits" holds). `$#v` is the element count. | `variable.c:1045`; `subst.sp` |

### Loads (L)

| # | Fact | Evidence |
|---|---|---|
| L1 | `pre_osdi f` (in a block or a `*#` line) loads before the circuit is parsed, relative to the deck's directory; a plain `osdi f` in the script runs after the parse, too late for the deck's devices; ngspice-45 has no `.pre_osdi` card (INV A35 is wrong). | `inp.c:791-797`, `:847-859`; `load_ctl.sp`, `load_hash.sp`, `load_late.sp`, `load_dot.sp` |

## 2. Verdicts, by command (`commands.c` `spcp_coms`, line of the entry)

Verdict words: **Analysis** (an `Analysis` through `read_run::read_analysis`), **State**
(the row named), **Output** (the outputs area's reader), **Inert** (an `Inert` opaque: moves
no number), **Script** (the whole block one `Script` opaque), **Private**, **Invalid**
(`InvalidInSource`: ngspice reports an error), **n/a** (not compiled into the pinned binary:
`no such command`, run). "Script if read" = Inert unless a later typed command reads what it
made (a vector, a variable, the current plot), then Script.

| Line | Command | Verdict | Why |
|---|---|---|---|
| 116 | `let` | Inert; `let @dev[p] = …` Script | makes a vector; Script if read (V3) |
| 120 | `reshape` | Inert | vector shape; Script if read |
| 124 | `define` | Inert | a vector function; Script if read |
| 128 | `set` | simulator option: State (`Option`/`Env` `FromHere`, S3); display variable: Inert; `units`, `filetype`, `csnumprec`, others: Private until classified | S3, V2 |
| 132 | `setcs` | as `set` | same `com_set` |
| 139 | `option` | State: `Option` (`Env` for `temp`/`tnom`) `holds: FromHere`; bare `option`: Inert (prints) | S3; `state_option_temp.sp` |
| 143 | `options` | as `option` | same `com_option` |
| 147 | `snsave` | Inert | writes a snapshot file |
| 151 | `snload` | Script | replaces the circuit state from a file |
| 155 | `circbyline` | Script | builds a circuit |
| 159 | `alias` | Script | renames commands: later lines mean something else |
| 163 | `deftype` | Inert | vector types |
| 168 | `bltplot` | n/a | TCL only; run: `no such command` |
| 172/177 | `plot` | Inert | display |
| 182 | `display` | Inert | display |
| 186 | `destroy` | Inert; Script if a later `meas`/`fourier` reads the current plot | plots |
| 190 | `setplot` | Inert; Script if a later typed command reads the current plot | selects the plot `meas` reads |
| 194 | `setcirc` | Script | switches circuits |
| 198 | `setscale` | Inert | display |
| 203 | `setseed` | Private | the random stream (`DrawWhen::Always` draws) |
| 207 | `transpose` | Inert | vectors |
| 211 | `gnuplot` | Inert | display (writes files) |
| 215 | `wrdata` | Inert | writes a file |
| 219 | `wrs2p` | Inert | writes a file |
| 223 | `hardcopy` | Inert | display |
| 227 | `asciiplot` | Inert | display |
| 231 | `write` | Inert (the backend's write-block rule reads its presence: backend note) | writes a raw file |
| 235 | `compose` | Inert | vectors; Script if read |
| 239 | `unlet` | Inert | vectors |
| 243 | `remzerovec` | Inert | vectors |
| 247 | `print` | Inert | display |
| 252 | `esave` | Inert | XSPICE event saves |
| 256 | `eprint` | Inert | display |
| 260 | `eprvcd` | Inert | writes a file |
| 264 | `edisplay` | Inert | display |
| 268 | `codemodel` | Private (`pre_codemodel`: Private) | an XSPICE library; no IR row |
| 274 | `osdi` | `pre_osdi`: `Load { kind: Osdi }`; plain `osdi`: Invalid (L1) | L1 |
| 280 | `use` | n/a | DEVLIB only; run: `no such command` |
| 285 | `load` | Script | reads result data |
| 289 | `cross` | Inert | vectors |
| 293 | `undefine` | Inert | functions |
| 297 | `op` | Analysis `Op` | `runcoms.c` `com_op` |
| 301 | `tf` | Analysis `Tf` | |
| 305 | `tran` | Analysis `Tran` | |
| 311 | `pss` | Analysis `Pss` | compiled in (run) |
| 319 | `sp` | Analysis `Sp` | compiled in (run) |
| 324 | `ac` | Analysis `Ac` | |
| 328 | `dc` | Analysis `Dc` | |
| 332 | `pz` | Analysis `Pz` | |
| 336 | `sens` | Analysis `Sens`/`SensAc` | |
| 340 | `disto` | Analysis `Disto` | |
| 344 | `noise` | Analysis `Noise` | |
| 348 | `listing` | Inert | display |
| 352 | `edit` | Script | edits and re-sources the deck |
| 356 | `mc_source` | Script | re-sources the deck |
| 360 | `dump` | Inert | prints the matrix |
| 364 | `fft` | Inert | a new plot; Script if read |
| 368 | `psd` | Inert | as `fft` |
| 372 | `fourier` | Output (the `.four` of the current plot) when exact (Q5); else Script | `dotcards.c` `.four` = `fourier` |
| 376 | `spec` | Inert | as `fft` |
| 380 | `meas` | Output (`Measure`) when exact (Q5); else Script | `com_measure2.c` on the current plot |
| 384 | `show` | Inert | display |
| 388 | `showmod` | Inert | display |
| 392 | `sysinfo` | Inert | display |
| 396 | `alter` | State: `Alter` (`ElementValue`/`SourceDc` bare, `DeviceParam` named) when the value is one literal or a loop word; else Unread (an expression) or Script (a vector) | S4 |
| 400 | `altermod` | State: `Alter` (`ModelParam`); `altermod … file` Script | S4 |
| 404 | `alterparam` | State: `Alter` (`Param`, `SubcktParam`) placed at the next `reset`; with no `reset` after it: Inert (changes nothing) | S2 |
| 408 | `resume` | Script | continues an interrupted run |
| 412 | `state` | Inert | display |
| 416 | `stop` | Script | a breakpoint ends an analysis early |
| 420 | `trace` | Inert | display during a run |
| 424 | `save` | Output (`Save`) only before any analysis of the script and with no `reset` after it (S1, Q6); else Script | `state_save_reset.sp` |
| 428 | `iplot` | Inert | display |
| 432 | `status` | Inert | display |
| 436 | `delete` | Script | removes a breakpoint, save or trace |
| 440 | `step` | Script | interactive stepping |
| 444 | `remcirc` | Script | deletes the circuit |
| 448 | `reset` | State: the restores of S1 (Q1) and the pending `alterparam`s | S1, S2 |
| 452 | `run` | the dot-card analyses at that point (Q2) | O3 |
| 456 | `aspice` | Script | an asynchronous run |
| 460 | `jobs` | Inert | display |
| 464 | `rspice` | Script | a remote run |
| 468 | `bug` | Inert | help |
| 472 | `where` | Inert | display |
| 476, 480, 484, 488 | `newhelp`, `tutorial`, `help`, `oldhelp` | Inert | help |
| 493 | `removecirc` | Script | deletes the circuit |
| 497 | `quit` | Inert at the block's end; Script before a later typed command (it ends the script) | |
| 501 | `source` | Script | runs another file at run time |
| 505 | `shift` | Inert; Script if read | variables |
| 509 | `unset` | display variable: Inert; simulator option: Private (if_option is not called back, unverified) | `variable.c:600-670` |
| 513 | `unalias` | Script | as `alias` |
| 517 | `history` | Inert | display |
| 521 | `echo` | Inert | display |
| 525 | `shell` | Script | side effects outside ngspice |
| 529 | `rusage` | Inert | display |
| 533 | `cd` | Script | changes where later files are read |
| 537 | `getcwd` | Inert | display |
| 541 | `version` | Inert | display |
| 545 | `diff` | Inert | display |
| 549 | `rehash` | Inert | shell path cache |
| 553 | `while` | Script | data-dependent flow |
| 557 | `repeat` | `repeat n` with a literal `n`: the body unrolled `n` times (`REPEAT_MAX`), else Script | `control.c` CO_REPEAT |
| 561 | `dowhile` | Script | data-dependent flow |
| 565 | `foreach` | over constant words: a `Loop` when exactly its law, else unrolled (`UNROLL_MAX`); else Script | V1, S5 |
| 569 | `if` | Script | data-dependent flow |
| 573, 577 | `else`, `end` | part of their construct | |
| 581, 585, 589, 593 | `break`, `continue`, `label`, `goto` | Script | data-dependent flow |
| 597 | `cdump` | Inert | display |
| 601, 605 | `mdump`, `mrdump` | Inert | write the matrix |
| 609 | `settype` | Inert | vectors |
| 613, 617, 621 | `strcmp`, `strstr`, `strslice` | Inert; Script if read | set variables |
| 626, 631, 637 | `fopen`, `fread`, `fclose` | Script | file input |
| 641 | `linearize` | Inert | a new plot; Script if read |
| 645 | `cutout` | Inert | as `linearize` |
| 649 | `devhelp` | Inert | help |
| 653 | `inventory` | Inert | display |
| 657 | `optran` | Private | ngspice's op-by-transient knobs; no IR row |
| 661 | `wrnodev` | Inert | writes a file |
| 666 | `check_ifparm` | Inert | self-test |
| — | any other word (`four`, `foo`) | Invalid | run: `<word>: no such command available in ngspice` (the CST's `control_command` list holds `four`: a scaffold bug) |

Totals over the 133 entries (132 names, `plot` twice), by the leading verdict: Inert 68,
Script 29, Analysis 11, State 8 (`set`, `setcs`, `option`, `options`, `alter`, `altermod`,
`alterparam`, `reset`), Output 3 (`save`, `meas`, `fourier`, when exact), Private 3
(`setseed`, `codemodel`, `optran`), Load 1 (`osdi` as `pre_osdi`), typed loops 2 (`foreach`,
`repeat`), `run` 1, `unset` 1 (split), construct words 2 (`else`, `end`), n/a 2 (`bltplot`,
`use`); `plot` counted once.

## 3. Rules (each with its fixtures under `tests/fixtures/dialect/ngspice/{read,write}/control/`)

| Rule | Evidence | Fixtures |
|---|---|---|
| C1 One script per deck: every block and `*#` line in file order (this file's, until includes are inlined); each statement's commands are items at its place; `.control`/`.endc` and `run` are framing | O1, O3, R49 (2) | `two_blocks`, `run_no_cards`, `run_with_cards`, `hash_line`, `continuation` |
| C2 Analysis commands are `read_run::read_analysis` rows; with no dot-card analysis they are the plan; with cards and no state change the cards stand once | R49 (3) | `analysis_commands`, `run_with_cards`, `meas_command` |
| C3 A state change in a deck with a dot-card analysis: every script statement one `Script` opaque, `P1404` | R49 (3), O4 | `script_state_cards` |
| C4 Flow (`P1401`), a reprinted read (`P1402`), an effect (`P1403`), an unroll past 4096 (`P1405`): every script statement `Script` | V2, V3, §2 | `script_flow`, `script_vector`, `script_set_var`, `script_effect` |
| C5 `alter dev v`, `alter dev = v`, `alter dev p v`, `alter @dev[p]=v` → `Alter` (`ElementValue` R/C/L, `SourceDc` V/I, `DeviceParam` named); `altermod` → `ModelParam`; a vector, an expression, a subcircuit path, `altermod … file` → `Unread` `P1408` | S4 | `alter_forms`, `alter_unread`, `altermod` |
| C6 `alterparam p=v` → `Alter(Param)` listed at the next `reset`; none after it → `Inert` `P1407`; `alterparam sub p=v` → `Unread` | S1, S2 | `alterparam_reset`, `alterparam_no_reset` |
| C7 `reset` with nothing to restore is framing; with state to restore `Unread` `P1408` until A27's restore row | S1 | `alterparam_reset` |
| C8 `option`/`set` of a typed key → `Option`/`Env` `FromHere`; an untyped circuit option or ngspice's own variable → `Private` `P1409`; any other variable, display → `Inert` `P1406` | S3 | `option_commands`, `inert_commands` |
| C9 `foreach v w…` over numbers whose pass is "set one target from `$v`, then a body reading `$v` only in display commands" → `Loop` sweep list `keep_last`, `name` = `v`; else unrolled with each word as written; `repeat n` unrolled | V1, S5, lead's Q3 | `foreach_loop`, `foreach_alterparam`, `foreach_inert_var`, `foreach_unrolled`, `repeat` |
| C10 `pre_osdi f` → `Load` osdi; plain `osdi` → `InvalidInSource` `P1411` | L1 | `pre_osdi`, `osdi_late`, `hash_line` |
| C11 A script `save` before the first analysis with no `reset` in the script, a `meas` after the script's one analysis of its kind (no card): the outputs area's rows; else `Unread` | lead's Q5, Q6 | `save_command`, `save_later_unread`, `meas_command`, `meas_command_unread` |
| C12 A word no command → `InvalidInSource` `P1411`; `setseed`, `optran`, `codemodel` → `Private`; a block in an `.if` branch → `Private` `P1410` | §2, lead's Q4, Q8 | `no_command`, `private_commands`, `in_branch` |
| W1 A deck whose run items hold an `Alter`, a `Loop`, a `FromHere` state item or a script-command pair is written as one script, analyses as commands; a `Load` is its own `pre_osdi` pair | R49 | `script_alter`, `script_mixed`, `opaque_pairs`, `load` |
| W2 `Loop` → `foreach <name> <values>`, its target set from `$name` (`alterparam` + `reset` for a parameter), the body, `end`; an unnamed loop's variable is `sr<depth>`, read back unnamed | V1, S2 | `script_loop`, `script_loop_unnamed`, `script_param_loop` |
| W3 Refused (`admits`): a parameter alter where another script state holds (`W1401`: its `reset` would drop it), a loop not over numbers (`W1402`), an alter value that is no number (`W1403`) | S1, S4 | — |

Census (ngspice-45 `examples/` and `tests/`, read in place; 404 files holding `.control`, each
statement its own file's script): no panic of this area; 256 read typed, 144 `Script` (94 flow,
31 effect, 16 reprinted read, 2 state with cards). Per command: 105 `meas` and 7 `save` outside
their exact case, 8 `alter` and 3 `alterparam` forms `Unread`; 15 `Private` (`set`, `option` of
untyped options, `pre_set`, `optran`); 2 `InvalidInSource`. Five files fail `ir::check` in other
areas (an XSPICE `xfer` model, an OSDI `N` instance, prose read as instances): sent to
`ng-scaffold`.
