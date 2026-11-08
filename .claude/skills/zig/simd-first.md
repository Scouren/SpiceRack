# SIMD-first: design the lane axis in

The **lane axis** is N independent instances of one transformation, run side by side in one vector register. It is an architecture decision made when the data layout is chosen, not a per-loop rewrite after a profile. Per-loop tactics (the five-part shape, reduce idioms, target flags) are the `simd-loops` skill's subject; this file decides where the lanes are and how the kernel is shaped and tested.

## Why the vector code must be written

Zig 0.16 and 0.17 ship with LLVM's loop vectorizer disabled to work around a miscompile (0.17.0 release notes; confirmed here: a plain `u32` sum and an `f32` map compile to scalar code at `-mcpu=x86_64_v3`, see [zero-cost.md](zero-cost.md)). 0.18 re-enables it (confirmed 2026-10-19 on 0.18.0-dev.349). Vector code comes from two places:

- **Per-element loops** over contiguous memory (map, reduction, search, count, compaction). On 0.18 these are plain loops written in the forms measured in the simd-loops skill's [zig.md](../simd-loops/zig.md) § Plain loops under 0.18: early exits split into 64-wide branchless blocks, max|x| as an integer max over the bit pattern, float sums in a gated `@setFloatMode(.optimized)` function. Written that way they matched or beat the stdpp SIMD library on every kernel measured, so don't add a SIMD iterator library. On 0.17 the same loops need the explicit five-part shape from simd-loops.
- **Explicit `@Vector(W, T)` kernels** for structure the vectorizer cannot see: one problem per lane. The vectorizer only makes lanes out of the iterations of one loop. It never runs N independent chains side by side, so the lane axis is always written by hand.

## Finding the lanes

Lanes are independent problems of identical shape: frequency points of an AC sweep, Monte Carlo or temperature sweep points, independent right-hand sides against one factorization, pixels, particles, rows.

These are not lanes, because each step reads the previous one: timesteps of a transient, Newton iterations, the harmonics of a harmonic-balance solve (coupled through the nonlinearity), a parser's state chain. The iterations run sequentially *inside* the kernel; the independent problems they solve are the lanes.

Done when the design names the lane axis, its typical count per call, and every axis that was rejected as a chain.

## The kernel shape

One kernel, generic over `W`:

- **The scalar oracle is the `W == 1` instantiation of the same kernel**, never a second code path. A hand-written scalar twin drifts; an instantiation cannot.
- **The kernel returns a per-lane failure mask** (`@Vector(W, bool)`) for lanes that did not converge, hit a singular pivot, or saw NaN or inf.
- **Failed lanes peel off to the scalar path**: the driver walks the mask bits and reruns those lanes through a robust scalar routine (a full re-pivot, a smaller step). The common case stays in vector registers.
- **The tail is the `W == 1` kernel**, not separate code.
- **`W` comes from the target**: `std.simd.suggestVectorLength(T) orelse 1`, resolved at compile time against `-mcpu`.

```zig
/// Newton's method for `x * x = a`, one problem per lane. The iterations are sequential
/// inside the kernel; the independent problems are the lanes. `W == 1` is the oracle.
fn Sqrt(comptime W: usize) type {
    return struct {
        const V = @Vector(W, f64);
        const iterations = 32;

        /// Writes every lane's root; returns the lanes that did not converge.
        fn solve(a: V, x: *V) @Vector(W, bool) {
            const half: V = @splat(0.5);
            const one: V = @splat(1);
            var g = @max(a, one);
            for (0..iterations) |_| g = (g + a / g) * half;
            x.* = g;
            const ok = @abs(g * g - a) <= @as(V, @splat(1e-12)) * @max(a, one);
            return !ok; // NaN compares false, so NaN and inf inputs fail
        }

        fn solveAll(as: []const f64, xs: []f64) void {
            assert(xs.len == as.len);
            var i: usize = 0;
            while (i + W <= as.len) : (i += W) {
                var x: V = undefined;
                const failed = solve(as[i..][0..W].*, &x);
                xs[i..][0..W].* = x;
                var mask: @Int(.unsigned, W) = @bitCast(failed);
                while (mask != 0) : (mask &= mask - 1) { // failed lanes peel off
                    const k = @ctz(mask);
                    xs[i + k] = fallback(as[i + k]);
                }
            }
            for (as[i..], xs[i..]) |a, *x| x.* = solveOne(a); // tail: the W == 1 kernel
        }
    };
}

fn solveOne(a: f64) f64 {
    var x: @Vector(1, f64) = undefined;
    return if (Sqrt(1).solve(.{a}, &x)[0]) fallback(a) else x[0];
}

/// The robust scalar path for lanes the kernel rejected.
fn fallback(a: f64) f64 {
    return @sqrt(a);
}

test "Sqrt(W) matches its W == 1 oracle" {
    const gpa = std.testing.allocator;
    const lanes = std.simd.suggestVectorLength(f64) orelse 1;
    const max_len = 3 * @max(lanes, 8) + 3; // past 3x the widest W tested
    const specials = [_]f64{ 0, -0.0, 1, 1e-310, 1e300, -4, std.math.inf(f64), -std.math.inf(f64), std.math.nan(f64) };

    const buf = try gpa.alloc(f64, max_len + 3);
    defer gpa.free(buf);
    const fast = try gpa.alloc(f64, max_len);
    defer gpa.free(fast);
    const slow = try gpa.alloc(f64, max_len);
    defer gpa.free(slow);

    var prng: std.Random.DefaultPrng = .init(0x5eed);
    const random = prng.random();
    for (buf, 0..) |*v, i| v.* = if (i % 5 == 0) specials[i / 5 % specials.len] else random.float(f64) * 1e6;

    inline for (.{ lanes, 8 }) |W| {
        for (0..max_len + 1) |n| for (0..4) |offset| { // every length, every start misalignment
            const as = buf[offset..][0..n];
            Sqrt(W).solveAll(as, fast[0..n]);
            Sqrt(1).solveAll(as, slow[0..n]);
            for (fast[0..n], slow[0..n]) |f, s| {
                if (std.math.isNan(s)) {
                    try std.testing.expect(std.math.isNan(f));
                } else {
                    try std.testing.expectEqual(@as(u64, @bitCast(s)), @as(u64, @bitCast(f)));
                }
            }
        };
    }
}
```

Checked on 0.17.0: the test passes in Debug, ReleaseSafe and ReleaseFast, and at `-mcpu=baseline`; it fails when the peel loop is removed and when the vector instantiation runs one iteration fewer than the oracle. `Sqrt(4)` at ReleaseFast `x86_64_v3` emits `vdivpd ymm`. `asm_probe.py` reads that pair as RESIDUE rather than NEGATIVE, because the oracle's scalar `@abs` lowers to `vandpd xmm`, which the probe counts as vector work; confirm vectorization by the `ymm` instructions in the kernel instead.

## The differential test

Every vector kernel gets one, in the owning module's test suite so `zig build test` runs it:

- **Oracle**: the `W == 1` instantiation (for a plain per-element loop: the naive scalar loop it replaced, in strict float mode).
- **Lengths**: every length from 0 to past 3× the widest lane count tested, so every tail size and the empty case run.
- **Misalignment**: several start offsets into one buffer.
- **Specials**: NaN, ±inf, ±0, a subnormal and a huge value, wherever the input can carry them.
- **Comparison**: bitwise when the kernel is lane-wise exact, as here; with a tolerance, or on inputs where reassociation is exact, when the vector version reorders a reduction.
- **Widths**: the target's `W` and a wider one (8 here), so a kernel correct only at one width fails.

Done when the test passes at every length, offset and width, and a deliberate break of the vector path (drop the peel, change one constant) turns it red.

## Sources

- Zig 0.17.0 release notes, "loop vectorization": https://ziglang.org/download/0.17.0/release-notes.html
- ESPice `AGENTS.md`, "Lane-axis doctrine"
- simd-loops [zig.md](../simd-loops/zig.md) § Plain loops under 0.18 (0.18-dev measurements against stdpp, 2026-10-19)
