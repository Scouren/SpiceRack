---
name: simd-loops
description: Turn a hot scalar loop into SIMD. Use when a for/while loop over a large array, slice, or buffer is a bottleneck; when the user mentions SIMD, vectorizing, lanes, intrinsics, or SIMD APIs; or when scanning, counting, comparing, or transforming many bytes or codepoints needs to be faster. Covers the general shape plus Zig and Rust.
---

Almost every "process N values at a time" loop compiles down to the same five-part **shape**. Learn to see the shape in a scalar loop and vectorizing it becomes about as routine as writing the loop was. The work is in the triage before it and the verification after.

## Triage the loop

Establish three facts before writing any vector code:

- **Element type and contiguity.** Lanes are loaded as a block, so the values being scanned must sit adjacent in memory.
- **Typical element count.** SIMD pays from a few hundred elements upward. Below that the scalar loop is the right answer, and saying so is a complete result.
- **The dependency between iterations.** Three cases:
  - _None_ — iteration N reads and writes only its own element. Vectorizes directly.
  - _Accumulator_ — every iteration folds into one running value (sum, count, min, max, all/any). Vectorizes as a vector accumulator reduced once at the end.
  - _Chain_ — each iteration's input is the previous iteration's output: `x = f(x)`, parser state machines, "am I inside a string yet". This is the blocker.

Done when you can name the element type, the typical length, and classify the dependency as none, accumulator, or chain. On a chain, report that and stop — resolving one takes techniques from the [out of scope](#when-the-shape-does-not-fit) section, not the shape.

## Flatten recursion and pointer-chasing

Only when the loop recurses or walks a linked structure. There are no lanes to fill when the next element's address isn't known until the current one has been read, so flatten first:

- Recursion becomes an explicit worklist or stack loop.
- Pointers become indices into one backing array.
- Array-of-structs becomes struct-of-arrays, so the one field being scanned is contiguous.

Done when triage passes on the flattened version. When the data resists being made contiguous, report that SIMD is off the table for this loop — a layout change is the prerequisite, and it is often the larger win anyway.

## Check the compiler first

Compile the scalar loop with optimizations on and read the generated assembly (godbolt, or `-S` / `--emit asm`). Compilers autovectorize simple arithmetic loops without complex control flow — but presence of vector instructions is not the bar: the autovectorizer often picks a weaker idiom than the hand-written shape (e.g. widening byte compares into word lanes, halving lane throughput), so benchmark the autovectorized loop against the speed target too.

**Zig depends on the version.** 0.16 and 0.17 ship with LLVM's loop vectorizer disabled, so a plain `for` loop stays scalar (unrolled 8×) and vector code is written by hand (the `zig` skill's `zero-cost.md` has the probes). 0.18 (`zig-overlay` master) turns it back on, and then a plain loop written in the right form matches or beats a SIMD library on maps, reductions and early-exit searches. The right forms, measured, are in [`zig.md`](zig.md) § Plain loops under 0.18. A strict float sum still stays scalar on every version, because vectorizing it reassociates; that is the one place gated fast-math pays. **Rust** autovectorizes integer and store loops, but not a float reduction under strict ordering; on stable, explicit SIMD goes through `fearless_simd` ([`rust.md`](rust.md)).

Hand-written SIMD earns its place when the autovectorized loop misses the speed target, or when the loop matters enough that the vectorization should be explicit and predictable — so an unrelated edit or compiler upgrade can't quietly turn it back into a scalar loop.

Done when you can state both whether vector instructions are present and whether the loop meets the speed target. Both yes → the work is done. Vectorized but slow → write the shape.

## Write the shape

Keep the scalar loop exactly as it is. It becomes both the tail and the fallback, so the vectorized version always has a reference implementation sitting next to it.

1. **Splat** each constant across every lane, and initialize any vector accumulator.
2. **Chunk** — loop while at least a full vector's worth of elements remain, advancing by the lane count. A partial vector is the tail's job, not this loop's.
3. **Lane op** — one operation, applied to every lane at once: compare, add, multiply, min, max, bitwise and. No inner loop appears.
4. **Reduce** — fold the vector result back into whatever the original loop produced.
5. **Tail** — the untouched scalar loop, handling the leftover elements and any target without vector support.

Parts 1, 2, 3 and 5 look nearly identical across completely different algorithms. Part 4 is where they diverge:

| The scalar loop was… | Reduce with |
|---|---|
| finding the first element that matches | mask → integer bitmask → count trailing zeros for the lane index |
| summing, counting, or taking min/max | a vector accumulator, folded to a scalar after the chunk loop |
| checking whether all/any elements qualify | a reduce-and / reduce-or to a single bool |
| writing a transformed output | a plain store of the result vector; no reduction at all |

Float `@max`/`@min` (Zig) and `f64::max` (Rust) return the non-NaN operand, so a max-reduction norm silently drops a NaN even in strict mode. A convergence norm built that way accepts a NaN update. Reduce the bit pattern instead ([`zig.md`](zig.md)), or check finiteness separately.

Read [`zig.md`](zig.md) before writing Zig vector code, or [`rust.md`](rust.md) before writing Rust — each carries the builtin and API names for all five parts, the lane-count query, worked examples of two different reduce kinds, and the target-CPU flags that decide whether any of it lowers to real vector instructions.

## Verify

Two checks, both required:

- **Differential test** against the retained scalar version on random inputs, sweeping lengths from 0 through at least three times the lane count so every tail boundary and the empty case get exercised.
- **Benchmark** both versions on input of realistic length. Keep results observable — `std.mem.doNotOptimizeAway` (Zig) / `std::hint::black_box` (Rust) — or the optimizer elides the work; a two-orders-of-magnitude "speedup" means it did. Zig 0.17 times through `std.Io`: `std.Io.Timestamp.now(io, .awake)`.

Done when outputs match at every length and the benchmark shows a speedup. A vectorized loop that measures slower is a normal outcome — report the number and keep the scalar version.

## When the shape does not fit

Some problems need SIMD techniques well outside the shape, and recognizing one early saves a wasted afternoon. Signals: elements of variable width so lane K's position depends on lanes before it (UTF-8, UTF-16); a genuine chain that survived triage; classification against many constants at once.

Each of those is a named **wall** — lane independence, or output shape. Before attacking the wall inside one stream, look for a lane axis across streams: N independent instances of the same chain (N circuits, N files, N parses) vectorize with no wall at all — the `zig` or `rust` skill's `simd-first.md` shapes that kernel. When there is only one stream, the wall-breaking techniques are shuffle-based classification, prefix scans, and data-dependent output length.

The published solutions are worth reading rather than reinventing — table-driven shuffle masks that realign ragged data into fixed lanes (simdutf), prefix-XOR via carry-less multiply and nibble-table classification with `pshufb` (simdjson). For text encoding and JSON specifically, linking one of those libraries beats writing it.
