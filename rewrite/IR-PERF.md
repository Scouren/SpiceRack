# IR throughput (slice S1)

Status: **preliminary, kernel kinds only** (2026-11-12). The circuit and run owners' rows are not
merged yet, so the generated designs hold the kernel's 11 item kinds and every expression form.
M5 re-measures with every feature once both owners land.

## How it is measured

```
cd crates/ir
cargo build --release --features gen --example throughput
target/release/examples/throughput 1000000 kernel
```

`examples/throughput.rs` generates a design of N items (`Size { items: N, scope_depth: 3,
expr_depth: 4, files: 1 }`, seed 1) and times, in one process, in a release build:

- **build**: `generate`, every row through `Builder`;
- **dump**: `text::dump` of every file;
- **parse**: `text::parse` of that text into a fresh design, including its final `link`. The
  example also times a `link` and a `check` of the parsed design on their own. It asserts that the
  text parses with no diagnostic, that the result is `eq_ignoring_spans` to the generated design,
  and that `check` reports nothing.

Machine: Intel Core i9-14900HX (32 threads, one used), 31 GB RAM, NixOS; rustc 1.95.0 inside
`nix develop`; the crate's default release profile.

## Numbers (best of the runs below; each run is one process)

| items | text | build | dump | parse (with link) | link alone | check |
|---|---|---|---|---|---|---|
| 1,000,000 | 42.4 MB, 1.10 M lines, 1.70 M expression rows | 1.31 s (765 k items/s) | 0.31 s (136 MB/s) | 0.87 s (**49 MB/s**) | 0.21 s | 0.39 s (2.6 M items/s) |
| 2,000,000 | 86.0 MB, 2.20 M lines, 3.41 M expression rows | 2.74 s (729 k items/s) | 0.64 s (134 MB/s) | 1.85 s (**46.5 MB/s**) | 0.45 s | 0.83 s (2.4 M items/s) |

Peak RSS at 10⁶ items, before the fixes below: 496 MB (`/usr/bin/env time -f %M`), for the
generated design, its text, the parsed design and its linked clone together.

## Against the budget (today's parser, 58 MB/s on the corpus)

Parse is **under the budget: 46–49 MB/s against 58 MB/s.** Where the time goes:

- About a quarter is the final `link` (0.21 s of 0.87 s at 10⁶ items). Without it the reading
  loop alone runs at about 65 MB/s. Today's 58 MB/s measures tokenize and parse only, with no
  semantic rows, no interning into a design and no binding, so the closer comparison is the
  65 MB/s.
- The rest is the line loop: interning every name, building every expression row, and the
  per-line allocations (`Cow` names, the shunting-yard stacks).
- The text is generated, not the corpus: short names, many small expressions. A corpus number
  waits for a reader that turns corpus decks into IR (slice S2).

## Fixes the measurement found (each was quadratic; each is now linear)

Before the fixes, a 10⁶-item design did not finish in 600 s. Each was found by timing every phase
at N and 4N.

1. `Builder::func`'s formal binding allocated a visited vector sized by the whole expression table,
   once per function: O(expressions so far) per `func`, in build and in parse. Now a set of the
   body's own rows.
2. `Gen::pick_def` scanned every definition on each read. Now a per-kind index caught up
   incrementally, then a few random draws tested for visibility.
3. `text::dump` scanned every definition for each `load` (its modules) and each `func` (its own
   definition). Now one map of modules per dump, and the `(scope, Func, name)` index.
4. `check`'s scope-owner test searched the parent's item list for every scope. Now one
   item→listing-scope map per check.
5. Literal validation formatted and re-parsed a string (`Literal::value`) on every literal, twice
   per parse. Finiteness is now decided from the digit count and the power, with `value` only at
   the boundary.
6. The interner and the definition index used SipHash. They now use FxHash, rustc's
   multiply-rotate hash, a few lines in `names.rs` with no dependency, including rustc-hash 2's
   final rotate. Without that rotate the multiply's poorly mixed low bits chose the buckets and
   the maps went quadratic, which a first try measured. The maps are lookup only, so output order
   is unaffected.

Scaling after the fixes, per phase (items/s or MB/s, 0.5 M → 1 M → 2 M): build 754 k → 707 k →
692 k items/s; parse 42 → 45 → 43 MB/s (SipHash) and 49 → 46.5 MB/s (FxHash). That is flat:
linear.
