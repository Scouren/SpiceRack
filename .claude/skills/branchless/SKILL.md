---
name: branchless
description: Replace a data-dependent branch with arithmetic or a select. Use when a hot loop contains an `if` on the data it is iterating; when the user mentions branchless, branch misprediction, cmov, conditional move, predication, or select; when perf shows a high branch-miss rate; or when a data-dependent branch is blocking vectorization.
---

A mispredicted branch costs ~20 cycles of flushed pipeline. Branchless trades that gamble for an unconditional cost paid every element — so the product is not a lower peak, it is a **flat** cost curve. Every idiom in the catalogue below is the same four-part **shape**; the work is the triage before it and the verification after.

## Triage the branch

The candidate is one specific thing: **an `if` inside a loop over bulk data whose condition reads that data.** A condition on the loop counter, a config field, or anything constant across the loop is memorized by the predictor and already costs nothing — leave it alone.

Two ways to establish the branch is actually unpredictable, in increasing cost:

**The hump test**, no profiler needed: run the loop on sorted input and on shuffled input. A branchy loop's cost is a hump — near zero at 0% and 100% selectivity, peaking around 50%. Sorting flattens the hump. A large sorted-vs-shuffled gap means an unpredictable branch; no gap means the predictor already has it and branchless will only slow you down.

**The counters**, when the loop is buried in a larger program:

```bash
perf stat -e cycles,instructions,branches,branch-misses ./prog
perf record -e branch-misses:pp ./prog && perf annotate --stdio -s hot_fn
```

Read the miss *ratio* per symbol, never the count — 10M branches at 0.1% is fine, 1M at 40% is the target. Below 1% overall, stop: bad speculation is not this program's bottleneck, and reporting that is a complete result. Above ~5%, or IPC under 1 alongside a high miss count, act.

Done when you can name the loop, its miss rate, and its rough selectivity. Guessing at selectivity is the failure here — measure it, because the whole decision below turns on it.

## Delete the branch before making it branchless

Cheaper than any mask, and each of these removes work rather than relocating it:

- **Split the collection.** `for (o) if (o.visible) draw(o)` becomes a `visible` array maintained on insert. Zero branches, better locality, no wasted iterations — existence-based processing from `/data-oriented-design`.
- **Sort the data** so the branch becomes predictable. Same loop measures ~14 cycles/element on random input and ~4 sorted.
- **Hoist the branch out of the loop.** A condition constant for the whole loop becomes a `comptime`/const-generic parameter and vanishes at compile time.

Done when the surviving branch genuinely varies per element and cannot be partitioned or hoisted away.

## The shape

```
1. CONDITION  produce the comparison as a value          c = x < t
2. WIDEN      to 0/1, or to an all-bits mask             @intFromBool(c) / (c as u64).wrapping_neg()
3. COMBINE    fold it in with arithmetic, a bit-blend,   v * c   |   v & mask
              or a select                                b ^ ((a ^ b) & mask)   |   cmov
4. COMMIT     always store, always advance —              out[n] = v;  n += c;
              the value or the index carries the
              decision, never the control flow
```

Step 4 is where the shape is won or lost. `if (p) out[n++] = v` becomes `out[n] = v; n += p` — the store is unconditional and the *index* holds the predicate. That costs one buffer sized for the full input; it is the memory-for-branches trade, and it is the single most transferable idiom here.

## The catalogue

Each row is a branchy tell you can grep for, and the shape it collapses to. Prefer the named intrinsic over the hand-rolled mask wherever one exists — it lowers to the same instruction and survives compiler upgrades.

| Branchy tell | Branchless form |
|---|---|
| `if (c) x = a else x = b` | `min`/`max`/`abs`/`clamp` builtin, a select intrinsic, or the blend `b ^ ((a ^ b) & mask)` |
| `if (p) n += 1` | `n += p` — bool widened to int, lowers to `adc`/`cinc` |
| `if (p) s += x` | `s += p * x`, or `s += p ? x : 0` |
| `if (p) out[n++] = x` | `out[n] = x; n += p` — always store, conditionally advance |
| `if (x < lo) x = lo else if (x > hi) x = hi` | `min(max(x, lo), hi)`, or the saturating operator |
| `if (a > b) swap(a, b)` | `t = min(a,b); b = max(a,b); a = t` — one sorting-network comparator |
| `i++; if (i >= n) i = 0` | `i &= n - 1` for power-of-two `n`, else `i -= n & -(i >= n)` |
| `if (i >= n) i = n - 1` before a load | `i = min(i, n - 1)` — also the Spectre-v1 mitigation shape |
| `if (p == null) …` in a loop | a sentinel or dummy object, so the body runs unconditionally |
| early `break`/`return` from a search loop | fixed trip count driven by length alone (branchless binary search, Eytzinger layout) |
| `if (x == A) … else if (x == B) …` per element | index a small `static const` table by the value |
| `switch (state)` per input byte | `state = transition[state][input]` — one 2-D load |
| `for (bit) if (v & 1 << bit) n++` | `@popCount` / `count_ones`, `@ctz`, `@clz` |
| `&&` / `\|\|` in a hot condition | `&` / `\|` when both sides are cheap and side-effect free |
| `%` or `/` by a runtime value | power-of-two mask, or a precomputed multiply-shift reciprocal |

Read [`zig.md`](zig.md) before writing Zig, or [`rust.md`](rust.md) before writing Rust — each carries the exact builtin names for all four steps of the shape, the compiler escape hatches for when the backend re-branches your mask code, and the panic/bounds-check pitfalls that quietly reintroduce branches.

## Where the branch still wins

Check every one of these before rewriting; each is a case where branchless measures *slower*, and the first four are common:

- **Predictability outside 25–75%.** The rule of thumb from Agner Fog and the one compilers use themselves: a predicted branch beats a conditional move above ~75% accuracy. A cmov costs its latency 100% of the time; a well-predicted branch costs nothing and speculates the downstream work for free.
- **The taken side is expensive** — a call, a division, a `sqrt`, an allocation, a syscall. Branchless executes both sides. Skipping expensive work is exactly what a branch is for.
- **The branch guards a load that could fault.** You cannot predicate past a bounds check. Either clamp the index instead (only if a wrong-but-safe read is acceptable), over-allocate so the out-of-range access lands in valid memory, or keep the branch.
- **The array is much larger than LLC.** The under-appreciated one: speculation past a branch issues loads early, so the branch is acting as a prefetcher. A cmov cannot be speculated, the load address waits on the condition, and memory-level parallelism collapses — a case measured at 0.55s branchless vs 2.92s branchy on a 4K working set inverting to the opposite result at 4M. Fix with explicit prefetch and batching independent queries, or keep the branch.
- **Long dependency chains.** A cmov serializes three inputs — both values and the flags — onto the critical path. Predication converts a control hazard into a data hazard; it is cheaper, not free.

## Verify

Three checks. The first is not optional: **the source-level transform is not the machine-level transform** — GCC and LLVM both re-branch hand-written mask code, LLVM's `X86CmovConversion` pass specifically converts cmov back to a jump, and Rust's binary search silently stopped being branchless at 1.25 from an LLVM codegen change alone.

- **Read the assembly.** `objdump -d -M intel`, `cargo asm`, `-femit-asm`, or godbolt with both variants side by side. The only `j*` in the loop body should be the loop-back edge on the induction variable; any jump whose condition derives from array data is a live branch. Look for `cmov`/`set*`/`sbb`/`adc`, or `csel`/`cset`/`cinc` on AArch64.
- **Re-run the counters.** The `branches` *count* should drop, not just the miss rate. Unchanged count means the compiler ignored you.
- **Sweep, don't sample.** Selectivity across 0–100% (branchless should be flat where branchy humps) and working-set size across L1/L2/L3/DRAM, because the winner flips with array size. Keep results observable — `std.mem.doNotOptimizeAway` / `std::hint::black_box` — and size the input past the predictor's memory: at N=1000 it memorizes the whole comparison sequence and the branchy version looks free.

Done when every variant's assembly is confirmed and the sweep is plotted. Branchless measuring slower is a normal outcome — report the numbers and keep the branch. Note also that flat can be the goal on its own: consistent latency is worth a worse mean in a real-time or interactive path.

## Then vectorize

Branchless is usually a waypoint, not the destination. A data-dependent branch is what blocks autovectorization — a vector body executes uniformly across all lanes, so any per-lane control flow must be if-converted to a mask before vectorizing is even legal. Writing the mask form by hand removes what the compiler would have to prove.

The shape survives the move up: bool becomes a lane mask, `-(bool)` is already the smear, the blend becomes a `select`, `idx += pred` becomes a compress-store. On the same loop, ~14 cycles/element branchy, ~7 branchless, ~0.3 vectorized. That last step is where the prize is, so hand off to `/simd-loops` rather than stopping at the scalar mask.

One legitimate hybrid: branch on the *whole vector* (a movemask or test-zero against all lanes) to skip a genuinely expensive block. One branch per 8 lanes predicts far better than one per element, and the lanes stay branchless.
