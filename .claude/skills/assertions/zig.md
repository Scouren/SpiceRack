# Assertions in Zig

Everything here was compiled with Zig 0.17.0 on x86-64 (`-mcpu=x86_64_v3`) and read with `asm_probe.py`.

## What `std.debug.assert` is

The 0.17 source, `lib/std/debug.zig`:

```zig
pub fn assert(ok: bool) void {
    @disableInstrumentation();
    if (!ok) unreachable; // assertion failure
}
```

So an assert is `unreachable` behind a branch, and its meaning follows the build mode:

| Mode | A false assert | What the optimizer does with a true one |
|---|---|---|
| Debug, ReleaseSafe | panics with `reached unreachable code` | keeps the compare and a cold panic call |
| ReleaseFast, ReleaseSmall | **undefined behavior** | treats the condition as a fact and deletes code that contradicts it |

The ReleaseFast row is a promise you make to the compiler. Probed:

```zig
export fn clamp_checked(x: u32) u32 {
    assert(x < 10);
    if (x >= 10) return 99;
    return x;
}
```

ReleaseSafe emits `cmp edi, 10; jae panic`. ReleaseFast emits `mov eax, edi; ret`: the `return 99` branch is gone, so `clamp_checked(50)` returns 50. An assert that can be false in production is a bug in ReleaseFast, not a missed check. Anything that can come from outside the program is an error return, per step 1 of the skill.

## What a ReleaseFast assert costs

The argument is an ordinary expression, evaluated before the call.

- A pure condition the optimizer can see through disappears. Probed: `assert(isSorted(xs))`, with `isSorted` a plain loop, folds to the same symbol as the function without it in ReleaseFast (verdict FOLDED); in ReleaseSafe it is 33 instructions against 10.
- An opaque call stays. Probed: `assert(validate(p, n))` with `validate` an `extern fn` still emits `call validate@PLT` in ReleaseFast, because the call may have side effects.

For an assert that is expensive even in a safe build, TigerBeetle's rule (`src/constants.zig`, doc of `verify`) is the one to copy: an unconditional O(1) assert before an O(N) loop; an O(N) check of an O(1) operation is guarded behind a comptime flag (`if (constants.verify) assert(...)`), and only once a benchmark shows it costs.

## Forms

```zig
const std = @import("std");
const assert = std.debug.assert;

const block_size = 4096;
const header_size = 128;
comptime {
    assert(header_size < block_size);
    assert(block_size % header_size == 0);
    assert(std.math.isPowerOfTwo(block_size));
}

const Header = extern struct { checksum: u64, size: u32, kind: u8, pad: [header_size - 13]u8 };
comptime {
    assert(@sizeOf(Header) == header_size);
}
```

A false `comptime` assert is a compile error: `error: reached unreachable code` with `note: called at comptime here` pointing at the line. Verified.

Pairing and negative space:

```zig
/// Copies the payload into `block` behind a header and returns the bytes used.
/// Asserts that the payload fits after the header.
fn writeBlock(block: *[block_size]u8, payload: []const u8) u32 {
    assert(payload.len <= block_size - header_size);
    const used: u32 = @intCast(header_size + payload.len);
    @memcpy(block[header_size..][0..payload.len], payload);
    std.mem.writeInt(u32, block[8..12], @intCast(payload.len), .little);
    assert(used <= block_size);
    assert(used >= header_size);
    return used;
}

fn readBlock(block: *const [block_size]u8) []const u8 {
    const size = std.mem.readInt(u32, block[8..12], .little);
    assert(size <= block_size - header_size); // Pair of writeBlock's precondition.
    return block[header_size..][0..size];
}

fn index(handle: u32, len: u32, valid: bool) void {
    if (valid) assert(handle < len);
    if (!valid) assert(handle >= len);
}
```

A capacity and a bounded loop:

```zig
const Queue = struct {
    const capacity = 64;
    items: [capacity]u32 = undefined,
    count: u32 = 0,

    fn push(q: *Queue, item: u32) void {
        assert(q.count < capacity);
        q.items[q.count] = item;
        q.count += 1;
        assert(q.count <= capacity);
        assert(q.count > 0);
    }
};

fn drain(q: *Queue, out: []u32) u32 {
    assert(out.len >= q.count);
    var iterations: u32 = 0;
    while (q.count > 0) : (iterations += 1) {
        assert(iterations < Queue.capacity);
        q.count -= 1;
        out[q.count] = q.items[q.count];
    }
    assert(q.count == 0);
    return iterations;
}
```

These blocks compile and pass `zig test` in Debug, ReleaseSafe and ReleaseFast as one file with a `test` per function.

To document that both outcomes of a condition are legal, TigerBeetle uses `stdx.maybe(ok)`, whose body is `assert(ok or !ok)`. It asserts nothing; it records that the negative space was considered.

In a `test` block, use `std.testing.expect*`, not `assert`. The std doc for `assert` says why: in fast and small modes it may not detect a test failure.

## Negative cost: the up-front length assert

Probe file (exported so `asm_probe.py` can find the symbols; `+%` keeps the overflow check out of the comparison):

```zig
export fn add_none(a: [*]const u32, na: usize, b: [*]const u32, nb: usize, o: [*]u32, no: usize) void {
    const as = a[0..na]; const bs = b[0..nb]; const os = o[0..no];
    for (0..as.len) |i| os[i] = as[i] +% bs[i];
}
// add_compound: same body after  assert(bs.len >= as.len and os.len >= as.len);
// add_split:    same body after  assert(bs.len >= as.len); assert(os.len >= as.len);
// add_for:      for (as, bs, os) |x, y, *z| z.* = x +% y;
```

```sh
zig build-obj -O ReleaseSafe -mcpu=x86_64_v3 -femit-asm=safe.s -fno-emit-bin a.zig
asm_probe.py safe.s add_compound=add_none add_split=add_compound add_for=add_compound --report
```

ReleaseSafe results:

| Variant | Static instructions | Panic sites | Loop |
|---|---|---|---|
| `add_none` | 23 | 2 (`outOfBounds` ×2) | rolled; `cmp r9, rax` and `cmp rcx, rax` every iteration |
| `add_compound` | 57 | 1 | 8× unrolled, no check inside; two `cmp`/`jb` before it |
| `add_split` | 58 | 1 | identical loop to `add_compound`; the extra instruction is the frame-pointer push moving from the panic path to the entry |
| `add_for` | 57 | 1 (`forLenMismatch`) | same loop; the entry checks are `jne` (lengths must be **equal**) |

Verdicts: `add_compound` vs `add_none` RESIDUE by static count, because the unrolled body is longer; the per-iteration checks are gone, which is the point. `add_split` vs `add_compound` RESIDUE by one instruction outside the loop. So the split form the skill asks for keeps the clean loop.

What did not change:

- No variant vectorized in ReleaseSafe or ReleaseFast (0 vector instructions); the win is unrolling and check removal, not SIMD. Wrapping the body in an `inline fn` with `noalias` slices did not change that either.
- With checked `+` instead of `+%`, the assert still removes the bounds checks (25 → 20 instructions, 3 → 2 panic sites) but the overflow check stays per element and the loop stays rolled.
- A read-only indexed loop (`s +%= as[i] *% bs[i]`) is hoisted without help: one `cmp` before the loop and an 8× unrolled body either way (61 vs 58 instructions with the assert).
- In ReleaseFast all four store variants are IDENTICAL (53 instructions, no checks), so the assert's codegen benefit is a safe-build benefit.

Choose `for (a, b, o)` when the lengths are equal by contract; choose the indexed loop with split asserts when one slice may be longer. For how to write such a probe and when a codegen win still needs a benchmark, follow the `zig` skill's `zero-cost.md`.
