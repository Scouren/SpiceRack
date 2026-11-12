# IR throughput (slice S1)

Status: **M5, every feature** (2026-11-12, `ir-rewrite` after the refs twin). The tables below
measure `FeatureSet::ALL` (all three owners' 25 item kinds, every expression form and every
placement) and, for comparison, the kernel set. The first measurement (kernel kinds only, before
the owners merged) is kept below it.

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

## Numbers after A18 and round 5 (2026-11-12, `ir-rewrite` 5f087948 and later)

Same machine and binary. The full set now also holds long literals (A18), `Placed`, and the
owners' rounds 4 and 5; the circuit owner's quadratic build (rounds 4 and 5) is fixed.

| set | items | build | dump | parse (with link) | link alone | check | peak RSS |
|---|---|---|---|---|---|---|---|
| all | 250,000 | 0.81 s (308 k items/s) | 0.126 s (137 MB/s) | 0.35 s (**49.6 MB/s**) | 0.07 s | 0.170 s (1.47 M items/s) | 189 MB |
| all | 1,000,000 | 4.19 s (239 k items/s) | 0.535 s (132 MB/s) | 1.45 s (**48.9 MB/s**) | 0.28 s | 0.889 s (1.12 M items/s) | 754 MB |
| kernel | 1,000,000 | 2.30 s (434 k items/s) | 0.31 s (134 MB/s) | 0.88 s (**47.3 MB/s**) | 0.24 s | 0.556 s (1.80 M items/s) | |

Against M5 at 10⁶ over every feature: build 4.0 → 4.19 s, dump 132 → 132 MB/s, parse 48.9 → 48.9
MB/s, check 0.91 → 0.889 s, peak RSS 751 → 754 MB (the long-literal pool). No phase regressed.
A first 10⁶ run on a loaded machine measured parse 36 MB/s and check 1.64 s; the two re-runs
above, minutes later, agree with each other and with M5, so the first is not reported.

**The scaling gate** (merge tier; `cargo run --release --features gen --example throughput --
scaling`): every phase over every feature at 10⁵ and 4·10⁵ items, best of two runs, ratio bound
8 (linear is about 4 with the cache, quadratic about 16); exit 1 over it.

| head | build | dump | parse | check | verdict |
|---|---|---|---|---|---|
| 68c6be2d (before the circuit fix) | 13.59 | 4.43 | 4.48 | 4.84 | FAIL (build) |
| 5f087948 (round 5) | 3.06 | 3.16 | 3.07 | 4.58 | pass |

It exists because the round-4 merge made the full-set build quadratic (170 s at 10⁶ became
100 s after one owner's fix, 4 s after both) and nothing in the gate saw it: IR-PERF stated
linearity as a measured fact, and a merge broke it silently.

## Numbers, every feature (M5; best of three runs at 10⁶; each run is one process)

`target/release/examples/throughput N all` and `N kernel`, same machine and profile as below.

| set | items | text | build | dump | parse (with link) | link alone | check | peak RSS |
|---|---|---|---|---|---|---|---|---|
| all | 250,000 | 17.4 MB, 0.26 M lines, 0.94 M expression rows | 0.77 s (325 k items/s) | 0.124 s (140 MB/s) | 0.34 s (**50.4 MB/s**) | 0.07 s | 0.168 s (1.49 M items/s) | 189 MB |
| all | 1,000,000 | 70.7 MB, 1.04 M lines, 3.76 M expression rows | 4.0 s (250 k items/s) | 0.54 s (132 MB/s) | 1.45 s (**48.9 MB/s**) | 0.29 s | 0.91 s (1.10 M items/s) | 751 MB |
| all | 2,000,000 | 142.2 MB, 2.08 M lines, 7.52 M expression rows | 9.9 s (203 k items/s) | 1.10 s (129 MB/s) | 3.01 s (**47.2 MB/s**) | 0.61 s | 2.52 s (0.79 M items/s) | 1.50 GB |
| kernel | 1,000,000 | 41.8 MB, 1.09 M lines, 1.59 M expression rows | 1.69 s (593 k items/s) | 0.30 s (140 MB/s) | 0.85 s (**49.4 MB/s**) | 0.22 s | 0.57 s (1.75 M items/s) | |
| kernel | 2,000,000 | 84.5 MB, 2.18 M lines, 3.17 M expression rows | 3.83 s (522 k items/s) | | | | 1.36 s (1.47 M items/s) | |

Parse over every feature runs at 47–51 MB/s against today's 58 MB/s; the breakdown below (a fifth
is the final `link`; the reading loop alone is about 62 MB/s on this text) holds for it too.

**`check` before and after the `refs` twin** (M5). Before, `check` read each row's references
through `map_refs` on one clone of the whole design; now `RowFacts::refs` reads them in place
(`kernel_gen::refs_and_map_refs_list_the_same_references_in_the_same_order` holds the twin to
`map_refs`, and goes red when one row's order is swapped).

| set, items | before (clone + `map_refs`) | after (`refs`) | peak RSS before → after |
|---|---|---|---|
| all, 250,000 | 0.304 s | 0.168 s (−45%) | 228 → 189 MB |
| all, 1,000,000 | 1.00 s | 0.91 s (−9%) | 908 → 751 MB (−17%) |
| kernel, 1,000,000 | 0.64 s | 0.57 s (−11%) | |

The after column also has A15's `ProbeContext`-through-reads pass, which before was not in
`check`, so the twin's own gain is larger than the difference shown. `check`'s instruction count
is linear (callgrind, kernel set: 290.7 M instructions at 60 k items, 1,158.4 M at 240 k, ×3.98
for ×4); its time per item rises from 0.67 µs at 250 k to 1.26 µs at 2 M because the design
outgrows the caches (the class table, the definition index and the expression rows are walked
in item order, not in table order). That is the next lever, not an algorithm.

**Build** over every feature is 250 k items/s at 10⁶ against 593 k for the kernel set: the
owners' generators make detail rows, pools and their names per item. It was quadratic in the
run owner's generators (170 s at 10⁶) until their fix; it is flat to within the cache effect
now (325 k → 250 k → 203 k items/s from 0.25 M to 2 M).

## Numbers, kernel kinds only, before the owners merged (best of the runs below; each run is one process)

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
