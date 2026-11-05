# Phase 7 (the new guarantees): status

Phase 7 of the rewrite (TESTING.md, "Golden gate"): the behaviour changes
the gate held back. Three branches landed together on `p7-integrate`:

- `p7-models` (`09ea795`..`698613a`): the writers walk a block's branches
  on a heap stack; Python's `pss` on Spectre, `add_fourier`'s count, a
  `log` `.step`; `render::models` (ngspice `r`/`c` models, the level-3
  diode, the R/C/D cards naming them, for Spectre and VACASK), W0112-W0114,
  the T-M decks, VACASK `scale`, `.options parser`.
- `development`'s `d6b4d72` (cherry-picked as `f20d84f`): a given deck's
  Spectre `montecarlo` over one analysis is `Ana::MonteCarlo`, Spectre's as
  before and VACASK's `mc` loop; the VACASK backend reads its per-sample
  raw files.
- `p7-resolve` (merged as `5171ebc`): Verilog-A modules as definitions in resolve
  (`DefKind::VerilogA`), the resolve walk in `resolve/walk.rs`, numbers in
  expressions in the target's suffixes (Spectre `2*1M` is ngspice
  `2*1meg`).

## Integration

- W-code renumber: `development` and `p7-models` each took W0112. W0112
  stays `p7-models`' Note ("a parameter ngspice ignores, not written");
  `development`'s Refusal ("a Monte Carlo the target varies wider",
  `named::vacask_bench`) is W0115, the next free code of the named
  writer's range (W0101-W0200). Its uses, its test
  (`a_region_montecarlo_is_spectres_and_vacasks_mc_or_refused`),
  `docs/simulate.md` and the changelog line say W0115. No golden input
  draws it, so `migration.xfail` names neither.
- `migration.xfail`: both phase-7 sections kept as written ("model kinds",
  "suffixes in expressions"); no input/target line is in both.
- `src/syntax/tests/render.rs`: both branches' tests kept (the `.step log`
  and `montecarlo` tests, the suffix read-back), the `given` helper last.
- `TESTING.md`: `p7-resolve`'s `src/syntax` row with `p7-models`' heap-stack
  wording for the deep `.if` deck.

## Gates

- golden: `golden record` (`/tmp/claude-1000/spicerack-golden/p7-integrate`)
  against `baseline-daa0ad0-fx13` (`--refusal-status-only`, `--skip
  verilog`): IDENTICAL except 545 listed xfails. The two branches'
  sections reach no common output: the combination is their union.
- `check-determinism`: IDENTICAL.
- fmt, clippy `-D warnings` (all targets, all features), `check_style.py`:
  clean.
- merge tier: 389 passed, 0 failed, 22 binaries.
- pytest: 521 passed, 2 failed. Both failures come from `p7-models`
  (`206db50`, `b7e9fd4`), not from the integration:
  `TestDialectText::test_a_fourier_harmonic_count_is_each_targets_words` and
  `test_a_log_step_is_spectres_sweep_and_refused_elsewhere` call
  `tb.netlist("xyce")`, and `Deck::netlist` (`src/python/deck.rs`) takes only
  HSPICE and the `RUNNABLE` dialects, so it raises
  `ValueError: unknown backend 'xyce'` where the tests expect the
  `RuntimeError` refusal.

## Left

- The two pytest failures above: either `netlist` writes Xyce (an API
  change: `docs/` and `skills/spicerack/` with it), or the tests check
  Xyce's refusal through Rust.
- `PHASE6-STATUS.md` "Left", the items phase 7 has not reached.
