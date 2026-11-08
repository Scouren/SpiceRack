# The shape in Zig

Zig has generic vectors as a language feature, so no intrinsics, no `unsafe`, and no per-architecture duplication. The compiler lowers `@Vector` operations to whatever the target instruction set provides.

## The five parts

| Part | Zig |
|---|---|
| lane count | `std.simd.suggestVectorLength(T)` → `?comptime_int`, null when the target has no useful vector for `T` |
| vector type | `@Vector(lanes, T)` |
| splat | `const c: V = @splat(value);` |
| load a chunk | `const v: V = slice[i..][0..lanes].*;` |
| lane op | ordinary operators — `+`, `*`, `>`, `&` — apply elementwise |
| compare result | `@Vector(lanes, bool)` |
| reduce | `@reduce(.And, v)`, `.Or`, `.Add`, `.Min`, `.Max`, `.Xor` |
| mask → integer | `const m: @Int(.unsigned, lanes) = @bitCast(cmp);` |
| locate a lane | `@ctz(~m)` for first false, `@ctz(m)` for first true, `@popCount(m)` to count |
| blend | `@select(T, pred, a, b)` |
| permute | `@shuffle(T, a, b, mask)` |

`std.simd` wraps the common reductions: `firstTrue(mask)` → `?index`, `lastTrue`, `countTrues`, `iota`. Reach for these before hand-rolling the bitcast, and hand-roll when the index arithmetic needs to be fused with something else.

Guard the whole vector section on the lane-count query so targets without vectors skip it and fall through to the tail:

```zig
if (std.simd.suggestVectorLength(T)) |lanes| { ... }
```

## Reduce kind: locate the first element

Consume codepoints until one is at or below `0xF`:

```zig
var end: usize = 0;

if (std.simd.suggestVectorLength(u32)) |lanes| {
    const V = @Vector(lanes, u32);
    const threshold: V = @splat(0xF);
    while (end + lanes <= cps.len) : (end += lanes) {
        const values: V = cps[end..][0..lanes].*;
        const above = values > threshold;
        if (@reduce(.And, above)) continue;
        const mask: @Int(.unsigned, lanes) = @bitCast(above);
        end += @ctz(~mask);
        break;
    }
}

while (end < cps.len and cps[end] > 0xF) end += 1;
```

The `@reduce(.And, ...)` fast path skips the bitcast entirely on full vectors, which is the common case when printable runs are long. The final line is the original scalar loop, doing double duty as tail and fallback.

## Reduce kind: accumulate

Counting occurrences, in the idiom the standard library itself uses in `std.mem.countScalar`:

```zig
var i: usize = 0;
var found: usize = 0;

if (std.simd.suggestVectorLength(T)) |block_size| {
    const Block = @Vector(block_size, T);
    const needles: Block = @splat(element);
    while (list.len - i >= block_size) : (i += block_size) {
        const block: Block = list[i..][0..block_size].*;
        found += std.simd.countTrues(needles == block);
    }
}

for (list[i..]) |item| found += @intFromBool(item == element);
```

For a float sum, keep a `@Vector(lanes, f32)` accumulator across the chunk loop and `@reduce(.Add, acc)` once after it, rather than reducing every iteration — the whole point is to keep the fold in vector registers. Mind accumulator width: `countTrues` folding into a `usize` is safe, but summing `u8` lanes into a `@Vector(lanes, u8)` accumulator overflows within a chunk — widen the accumulator vector (e.g. `@Vector(lanes, u32)` for `u8` data).

## Plain loops under 0.18

With 0.18's loop vectorizer on, try a plain loop in one of these forms before writing `@Vector` code. Measured 2026-10-19: 0.18.0-dev.349, ReleaseFast, `-mcpu=native` on an i9-14900HX (AVX2), f64, best of 31 runs, ns/elem at n = 1024 / 16384. "Library" is stdpp, an explicit-SIMD iterator library.

| Loop | Form | Plain | Library |
|---|---|---|---|
| map, axpy, elementwise write | strict `for`, branchless body | 0.078 / 0.19 | 0.11 / 0.23 |
| integer count | `n += @intFromBool(p)` | 0.058 / 0.11 | 0.19 / 0.20 |
| float sum, dot, ‖x‖² | fast-math function + result gate | 0.079 / 0.167 | 0.094 / 0.151 |
| max\|x\| (∞-norm) | i64 max of the bit pattern | 0.153 / 0.152 | 0.140 / 0.138 |
| find first NaN | 64-wide blocks of integer OR, one branch per block | 0.092 / 0.120 | 0.137 / 0.170 |
| all-within-tolerance (convergence) | 64-wide blocks of `!(a <= b)` OR | 0.155 / 0.20 | 0.234 / 0.264 |

The same loops written the obvious way: strict float sum 0.50, float `@max` reduction 0.185, early-return NaN search 0.41, early-return convergence check 0.56.

**Early exit: block it.** The vectorizer will not touch a loop with a data-dependent `return` inside. Make each full block branchless, and branch once per block:

```zig
var i: usize = 0;
while (i + 64 <= dx.len) : (i += 64) {
    var bad: u64 = 0;
    for (dx[i..][0..64], x[i..][0..64]) |d, v| bad |= @intFromBool(!(@abs(d) <= rel * @abs(v) + abs));
    if (bad != 0) return false;
}
var bad: u64 = 0;
for (dx[i..], x[i..]) |d, v| bad |= @intFromBool(!(@abs(d) <= rel * @abs(v) + abs));
return bad == 0;
```

Write the test as `!(a <= b)` so a NaN fails it at no extra cost. To find where the hit is, rescan the block that hit with the scalar loop.

**max|x|: reduce the bits, not the float.** With the sign bit cleared, an f64 bit pattern is monotone in |x|, and a NaN's pattern sorts above Inf. So the integer max is exact and keeps the NaN:

```zig
var m: i64 = 0;
for (x) |v| m = @max(m, @as(i64, @bitCast(@as(u64, @bitCast(v)) & 0x7fff_ffff_ffff_ffff)));
return @as(f64, @bitCast(m));
```

Use `i64`, not `u64`: AVX2 has a signed 64-bit compare (`vpcmpgtq`) but no unsigned max (0.15 vs 0.19 ns/elem). Below SSE4.2 it is still correct, only scalar. For signed min/max, map each value to the monotone key `k = i ^ ((i >> 63) & 0x7fff_ffff_ffff_ffff)`. Float `@max` reductions do not vectorize even under fast-math (0.185 either way).

**Float sums: gated fast-math.** A strict float sum stays scalar because vectorizing it reassociates. `@setFloatMode(.optimized)` permits that, and it is the only place in the table where fast-math changed anything (6×). It also grants `nnan`/`ninf`, so:

1. Put the loop alone in a small function. The mode covers the enclosing scope.
2. Gate in the strict caller. `const r = dotFast(a, b); if (!std.math.isFinite(r)) return dotStrict(a, b);` works when NaN/Inf only has to survive. In the measurement a NaN or Inf input did come out non-finite, but `nnan` does not promise that. Where a NaN input must be *detected*, gate the input with the blocked integer scan (0.09 ns/elem), or rely on a guarantee from upstream. Name the gate in a `// fast-math:` comment.
3. Never test for NaN inside an optimized scope. Measured: `v != v` on a NaN returned **false** there, because LLVM deleted the compare. `std.math.isNan` and bit-pattern tests survived, because they are integer operations.
4. The reassociated result changes with lane width and target: compare it to the strict sum within a tolerance, never bitwise.

**When a NaN must come out of the sum, skip fast-math.** Under `nnan`, a NaN input is not guaranteed to reach the result, so a gate that must catch every NaN has to scan the input. Measured in ESPice's core, that scan cost more than fast-math saved: a gated dot ran at about 0.5 cycles/entry against 0.18 for a strict loop. Instead, write a strict loop with independent accumulators that the vectorizer can map to vector registers. 16 `f64` accumulators fill 4 AVX2 vectors. Combine them in a fixed pairwise order at the end. That ran within 15% of ungated fast-math, propagates NaN and Inf exactly with no gate, and gives the same bits on every run for a given target. This is the one case where multiple accumulators are the answer on 0.18, not a 0.17 workaround. Keep the accumulators as a `[16]f64` array combined pairwise, not a `@Vector(16, f64)` finished with `@reduce(.Add, acc)`. In strict mode a float `@reduce(.Add)` is an ordered, sequential chain of scalar adds (`vextractf128` + `vaddsd`). Measured on sum: 0.109 vs 0.145 cycles/elem at n=1024, and 0.44 vs 0.56 at n=16. Confirm from the assembly that the accumulators became packed `ymm` adds.

Fast-math did not help maps, counts or compares, and it made the blocked convergence check slower (0.28 vs 0.155). Keep those strict.

**How the mode is scoped.** Read off the assembly, 0.18.0-dev.349:

| Code | Result |
|---|---|
| `@setFloatMode(.optimized)` at the top of a function, loop in the body | vectorized (ymm `vfmadd231pd`, 4-wide) |
| the same inside a labeled block `blk: { @setFloatMode(.optimized); ... break :blk s; }` in a strict function | vectorized, folded into the same body: block scope works, and the rest of the function stays strict |
| optimized loop whose arithmetic is in an `inline fn madd(s, x, y)` defined at file scope | **scalar**: folded into the strict version. The helper body keeps its own (strict) mode even when inlined |
| `v != v` written directly in an optimized scope | deleted: compiles to `xor eax, eax`, always false |
| `v != v` in a strict `inline fn`, called from an optimized scope | kept (`vucomisd` + `setp`) |
| `std.math.isNan(v)` in an optimized scope | kept |

The mode applies to the code lexically inside the scope, not to calls out of it. So the reduction's `+` and `*` must be written in the optimized scope itself. A strict helper called from there keeps its NaN checks, and also keeps its arithmetic scalar.

**Removing old workarounds.** On 0.17, a fast float reduction meant hand-written `@Vector` code: N accumulators, `@reduce(.Add, ...)`, a scalar tail. On 0.18 a plain loop in an optimized scope compiles to the same thing: 19 vector instructions against 18 for the hand-written 4-accumulator `@Vector` dot, both ymm FMA loops. Hand-written multi-accumulator scalar unrolling, used to break the add dependency chain, is obsolete the same way, except for a strict sum whose NaN must reach the result (above). Delete the workaround only after the replacement passes both checks below. Keep explicit `@Vector` where the lanes are not a single loop's iterations.

**Verify it is still vectorized.** A timing alone can mislead: LLVM may hoist a pure call out of the benchmark loop (a 0.0002 ns/elem result means it did). Read the code instead:

1. Put the kernel in an `export fn` that takes `[*]const T` plus a length (a C-ABI signature, so the symbol survives), next to the version it replaces.
2. `zig build-obj -OReleaseFast -mcpu=x86_64_v3 -fstrip -femit-asm=k.s -fno-emit-bin k.zig`. Pin the target CPU rather than using `native`, so the verdict reproduces on another machine.
3. Count packed instructions per function. The `zig` skill's `scripts/asm_probe.py k.s new=old --report` prints instruction, vector, call and branch counts per pair. Look for `ymm` operands and `…pd`/`…ps` packed mnemonics in the loop body; scalar code uses `…sd`/`…ss` on `xmm`.
4. Watch for folding. When two functions compile to identical code, LLVM emits one body and the other symbol as an alias (`dot_strict = .Lprobe.dot_opt_inline_helper`). The probe reports such a pair as FOLDED, and in this case that verdict was the bug: the "optimized" version had become the strict one.

Done when the replacement has vector instructions in its loop body, it does at least as well as the code it replaced, and the differential test against the strict scalar loop passes. Make the check permanent where the project has a codegen-probe step, so a later edit cannot quietly turn the loop scalar again.

**Still write `@Vector` by hand** for gathers, shuffles, lane-dependent state, using the five-part shape above. Also write it by hand when the output legitimately aliases an input (an in-place update called as `f(x, x)`). A plain loop gets a runtime overlap check, and when the arrays overlap it falls back to the scalar copy (measured in ESPice's transient companion update). When the slices never overlap, mark them `noalias` instead. The vectorizer only handles independent lanes over contiguous memory.

## Gotchas

- **Vector length is independent of register width.** A vector shorter than the native SIMD width typically becomes one instruction; longer becomes several; an operation with no SIMD support becomes element-at-a-time scalar code. Powers of two from 2 to 64 are the useful range.
- **`suggestVectorLength` resolves at compile time against the compilation target.** Building with `-mcpu=native` bakes in the current machine's feature set, so a binary distributed to an older CPU will fault. Pick an explicit baseline `-mcpu` when shipping.
- **Comparisons produce `@Vector(n, bool)`, not an integer mask.** `@bitCast` to `@Int(.unsigned, lanes)` is the bridge, and it only works when `lanes` matches the integer's bit width.
- **The chunk loop condition must guarantee a full vector.** `slice[i..][0..lanes].*` on a short remainder is a compile error at best and an out-of-bounds read at worst; `while (i + lanes <= slice.len)` is the guard.
