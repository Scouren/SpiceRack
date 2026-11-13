# Slice S1, Testing and Implementation phases: the IR crate, working

For the three owners of `crates/ir` (kernel, circuit, run). The definitions are frozen: do not
change a `pub` signature, a row's fields or an enum's members without a ruling (ask in your
report; a small addition another owner needs goes through the kernel owner). `IR-KERNEL.md`,
`IR-CIRCUIT.md`, `IR-RUN.md`, `IR-DECISIONS.md` and `RULINGS.md` are binding.

## 1. Order of work, per owner

1. **Tests first.** Write the tests of §2 for your files against the frozen signatures. Run
   them: they must compile and **fail** on the `todo!()` bodies (a test that passes on a stub
   tests nothing). Commit that red state (`crates/ir: <owner> tests, red on stubs`), with the
   failing test names in the commit body.
2. **Bodies.** Replace every `todo!()` in your files. Simplest code that is correct: no new
   abstraction, no new dependency (the crate has none and keeps none; the test PRNG is the
   generator's own), explicit stacks wherever depth follows the input (C11).
3. **Assertions.** Every function states its contract: about two assertions per function where
   they say something real (a handle in range, parallel columns of equal length, a capacity, an
   exit invariant). An operating error (bad IR text, an invalid design) is a diagnostic row; a
   bug is an assertion. Input is validated once, at `Builder` and at `text::parse`.
4. Remove the definition-phase `#![allow(unused_variables, dead_code)]` once your files need it
   no longer (the kernel owner removes the crate-level line last).

## 2. What the tests are (each names its oracle)

| Test | Oracle | Owner |
|---|---|---|
| Literal: `value()` of hand-picked literals against the decimal's correctly rounded double; `from_f64(v).value() == v` for a fixed list and for 10⁶ generated doubles; `eq_value` on `1k` vs `1e3` vs `1000`; every invariant has a literal that breaks it | spec example; inverse | kernel |
| Names, pools, tables, scopes, definitions: what goes in comes back by handle; ids are first-seen order; capacities assert | inverse; model (a `Vec` of what was pushed) | kernel |
| Expressions: each tag encodes and decodes to the same `Expr`; children below parents | inverse | kernel |
| Evaluator: every `Func`, `BinOp`, `UnOp` at hand-worked points, including each stated domain edge and each native-meaning pair where they differ (`Pow`/`PowAbs`/`PowRe` at a negative base, `Exp`/`ExpLin14` at 14 and beyond, the two roundings at halves) | spec example: the value derived by hand in the test's comment, never by calling the function another way | kernel |
| `check`: for every `Violation`, a design built to break exactly that invariant is reported with that code and nothing else; a valid design reports nothing | hand-broken fixture, one per variant (names compared against `Violation::ALL`) | each owner for its rows; kernel for the rest |
| IR text: hand-written text of every item kind parses to the rows the test builds by hand; `dump` of those rows is the canonical text; the worked example of `text.rs` | spec example | each owner for its rows |
| Round trip: `parse(dump(d))` equals `d` (`eq_ignoring_spans`) for 2,000 generated designs per seed list, every feature on | inverse, generated | kernel (harness), all (rows) |
| Generator: every generated design passes `check` and uses only the requested features, for the full set and for 20 random subsets | the checker, which was tested above against hand-broken designs | kernel (harness), all (rows) |
| Features: `features_used` of a hand-built design is the hand-listed set; each `Extra` has a row that sets it and a neighbour that does not | spec example with a legal neighbour | each owner |
| Linker: a forward reference, a shadowed name (inner scope wins), `name?` stays unresolved, a kind mismatch does not bind | hand-built designs | kernel |
| Depth: an expression and a scope nest 10⁴ deep through build, dump, parse, check, eval, in a debug build | no stack overflow; equality with the built design | kernel |
| Determinism: building the same calls twice gives byte-equal dumps | byte compare | kernel |

Seeds are fixed in the test and printed on failure. A failing generated case is shrunk by the
generator's size parameter and checked in as a hand-written regression.

## 3. Gate

```
nix develop . -c bash -c 'cd crates/ir && cargo fmt --check && cargo clippy --all-targets --features gen -- -D warnings && cargo test --features gen 2>&1 | tail -5 && cargo doc --no-deps 2>&1 | tail -2'
python3 .github/scripts/check_style.py crates/ir/src crates/ir/tests
```

Plus, once, by the kernel owner at the end: throughput of `build` (a generated 10⁶-item design),
`dump` and `parse` in a release build, written into `rewrite/IR-PERF.md` with the command and
the machine (the budget to beat is today's parser, 58 MB/s on the corpus; say plainly if a
number is under it and where the time goes).

## 4. Commits and report

Commit at each green step on your branch, staging by name. Final report: the hashes, test
counts by table row above, anything in the definitions that the bodies showed to be wrong (do
not fix a frozen signature silently: list it), and the gate's output tails.
