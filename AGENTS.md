# Working in this repository

The human designs; you type. Work in layers: finish one, show it, and wait for approval before going one level deeper. Every step ends on its "Done when".

Read `codebase-rules` before starting a module: it orders the skills below and settles their conflicts. Run `setup-matt-pocock-skills` once so `to-spec`, `to-tickets` and `triage` know the tracker. In their domains, `compiler-construction`, `gui-architecture`, `spice-engine` and `writing-perfect-verilog` apply on top of every step.

## Add a feature

1. Grill. Run `grill-with-docs` (it runs `grilling` and `domain-modeling`) and name the oracle for every output. Too big for one session: `wayfinder`. A question only running code can answer: `prototype`. Done when the human confirms a shared understanding; then `to-spec` and `to-tickets`.
2. Blocks. `codebase-clean-code` steps 1 to 4 (data, owners, spines, seams), the six questions of `data-oriented-design`, and `test-plan` for each bug class's oracle and layer. Done when the human approves the tables, the ownership map and the seams.
3. Definitions. Every type, field and signature, each body `todo!()` or `@panic("todo")`; `api-design` at any boundary another program calls; `ponytail` throughout. Done when it compiles and the human approves.
4. Seams. Public bodies call other units, placed with `codebase-design`, each call carrying a `doc-comments` comment and `assertions` at entry and exit. Done when the human approves the list of seams under test.
5. Private functions. Per seam, one `tdd` slice: write the test and watch it go red, propose the algorithm and its data movement, implement once approved, watch it go green. In `.zig` or `.rs`, the `zig` or `rust` skill; a hot branch, `branchless` then `simd-loops`; GPU work, `gpu-parallel-algorithm-thinking` then `gpu-kernel-optimization`. Done when every agreed seam has a green test.
6. Seam checks. Pair every assertion across its seam, and add the property, differential, fuzz and exhaustive tests `test-plan` named. Done when every bug class names a test file.
7. Final pass. `code-style`, the `doc-comments` audit, `code-review`, then `ponytail-review`. Done when the gate passes.

A ticket with no design decision left goes through `implement` in one piece.

## Fix a bug

1. Reproduce with `diagnosing-bugs`. Done when one command goes red on the bug.
2. Find the root cause: grep every caller of the function you will change, and fix it where they all route through. Done when the command goes green.
3. Add the oracle that would have caught it (`test-plan`): a property, a reference or a generator feature. Done when it goes red on the old code.
4. Final pass, as in feature step 7.

## Change a feature

1. State the change radius: run a `codebase-clean-code` change drill and list every existing function body the change edits. Done when the human accepts the number.
2. Pin the current behaviour. Untested code gets `legacy-code` characterization tests at the seam first; restructure in `refactoring-patterns` steps. Done when the tests pass on the current code.
3. Re-enter the feature steps at the highest layer the change touches: a new seam at step 4, a new algorithm at step 5. A rewrite is `rewrite-clean`, a port is `clean-room-pseudocode-pipeline`, and a whole repository is `codebase-improvement` or `improve-codebase-architecture`.

## Rulings

- Data layout beats code brevity; in hot code, small functions yield to it.
- `ponytail` decides how little code to write, never the shape of the data.
- A seam at ladder rung 5 or above enters hot code only with a zero-cost probe verdict.
- Measure every performance change; a vector loop that benchmarks slower is reported, and the scalar loop stays.
- Keep every NaN gate, allocation-failure path, `errdefer`, trust-boundary validation and numerical tolerance.

## The gate

Done means: `check_style.py` exits 0; the suite passes in a safe build (Zig Debug or ReleaseSafe, Rust with debug assertions); touched files average two assertions per function; every rung-5+ seam has its probe verdict; the doc audit ran after the code. While iterating, run only your unit's tests, and use `long-term-garbage-collection` when builds queue or caches grow. Then `pr` for the description, `resolving-merge-conflicts` when the merge fights back, and `handoff` before the context fills. Repository-wide: `ponytail-audit` for bloat, `ponytail-debt` for deferred shortcuts.

## Skills

Every skill named above is vendored in `.claude/skills/` (Claude Code loads them from there;
other agents read `.claude/skills/<name>/SKILL.md` and the files it names). Source:
https://github.com/OmarSiwy/NixOSConfig/tree/main/skills. Update by re-copying from there, never
by editing the copy.

## SpiceRack

Build SPICE circuits in Python, netlist them for any simulator (ngspice, ESPice, HSPICE, Xyce,
LTspice, Spectre, VACASK, CDL, Verilog), run them and read the results back. A Rust crate
(edition 2024) with PyO3 bindings shipped as the `spicerack` package. `README.md` is the overview.
The reader and writer are `src/syntax/`: tokenize → parse → AST → resolve → render, one AST every
dialect reads into and every dialect is written from; dialects are declared definitions in
`src/syntax/dialects/*.rs` (no `match Dialect`). `TESTING.md` holds the golden gate and the tiers.

Owners: `import` reads files, `syntax` reads text into the AST and writes it in any dialect,
`backend` runs one simulator each, `src/python/` + `python/spicerack/` is the Python API
(backend neutral, PySpice-close). In this repo `compiler-construction` applies to `src/syntax`,
`spice-engine` to reading a simulator's result, `simd-loops` to `src/rawfile.rs`.

### Build

- Every build runs inside `nix develop -c …`; `nix develop .#testing` adds ngspice, Xyce,
  OpenVAF, VACASK, ESPice and the sky130/gf180 PDKs the integration tests run against.
- Rust: `cargo test` (`--test <file>` while iterating). Python: a venv with
  `--system-site-packages`, `maturin develop --target-dir target/py` (its own target dir,
  as rust-analyzer has: it builds another crate type for another Python), `python3 -m pytest`.
- Flags live in `.cargo/config.toml`, never a one-off `RUSTFLAGS`.
- One cargo process per agent; don't build while `free -g` shows under 6 GB available or
  `df -h /` under 30 GB free.
- Gate on exit codes: `cmd | tail` swallows them, use `exit "${PIPESTATUS[0]}"`.

### Test

- Every test names its oracle: a netlist is checked by running it on the real simulator or
  against a golden with provenance, never by re-deriving it the way the writer does.
- Unsupported fails and never skips: a missing simulator in `.#testing` is a failure. A known
  failure is a named marker; an unexpected pass fails until the marker goes.
- A golden that disagrees with an independent oracle is a bug in the code that wrote it: fix
  the code, regenerate the golden, say so in the commit.
- The golden gate: output bytes against the baseline with reviewed `migration.xfail` lines;
  determinism (1 job = N jobs gives the same bytes). A performance change has zero golden diff.

### Commit

- Commit at every green step: both suites passing, `cargo fmt --check`, clippy `-D warnings`,
  `check_style.py src tests` clean. The message says what changed and in which owner.
- Stage files by name, never `git add -A`; run simulators in a temp dir.
- A change to the public Python API updates `docs/` and `skills/spicerack/` (the user-facing
  skill) in the same commit.
- `main` moves only through a pull request whose merge result passed the checks. Never commit
  red to `main`; never push without the human's say.
