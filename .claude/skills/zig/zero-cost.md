# Zero-cost: prove there is no residue

**Residue** is the run-time work an abstraction leaves behind beyond the code you would have written by hand: an extra call, an indirect call, a copy, a check, a lost vectorization. An abstraction is admitted on a hot path only if it leaves zero or negative residue, and the evidence is a codegen probe, not a belief about the optimizer.

## The probe protocol

This is Gompute's method (`tests/codegen.zig` + `tools/codegen_check.zig`), generalized by `scripts/asm_probe.py`.

1. **Write the pair.** Copy [`scripts/probe_template.zig`](scripts/probe_template.zig) to `tests/codegen.zig`. For each abstraction write `export fn gen_<name>` (through the abstraction) and `export fn hand_<name>` (the loop without it), same signature, runtime pointer + length parameters. Done when both names appear in the file and neither calls a shared helper.
2. **Emit the assembly.** `zig build-obj -O ReleaseFast -mcpu=x86_64_v3 -femit-asm=probe.s -fno-emit-bin tests/codegen.zig`. Done when `probe.s` exists.
3. **Read the verdict.** `python3 scripts/asm_probe.py probe.s gen_<name>=hand_<name>`. Done when every pair prints FOLDED, IDENTICAL, EQUIVALENT or NEGATIVE.
4. **Wire it into `zig build test`** with the snippet below, so a regression fails the build. Done when breaking the `gen_` body on purpose turns `zig build test` red, and restoring it turns it green.
5. **Benchmark before claiming speed.** The probe proves the absence of residue; a speedup claim needs a timed before/after (median of repeated runs, results kept alive with `std.mem.doNotOptimizeAway`). Done when every speed claim in the change carries a number.

| Verdict | Meaning |
|---|---|
| FOLDED | LLVM merged the two bodies into one symbol: the strongest result |
| IDENTICAL | same instructions after label and constant-pool normalization |
| EQUIVALENT | same mnemonic sequence, different registers |
| NEGATIVE | `gen` does strictly less static work, or is vectorized where `hand` is scalar |
| RESIDUE | `gen` leaves extra work; exit status 1 and a side-by-side diff |

Reading the counts:

- They are **static**: instructions in the listing, not instructions executed. Unrolling raises the static count while cutting per-iteration work, so a loop that gains an 8× unroll can read RESIDUE. When the verdict is RESIDUE on a loop, read the diff's inner loop block (between the backward branch and its target) and compare that.
- A `noinline` helper appears as `jmp helper`. Name the helper as its own pair (`helper=hand_x`); probe symbols may be any function in the file, not only exports.
- An `export fn` cannot take a Zig-only type (a `*const fn` without `callconv(.c)`, a slice). Pass `[*]T` plus a length and rebuild the slice inside.

### build.zig wiring

Verified with `zig build test` on 0.17.0: green on FOLDED, red with the RESIDUE table printed when the `gen_` body regresses.

```zig
// build.zig — inside `pub fn build(b: *std.Build) void`, next to the unit tests
const probe = b.addObject(.{
    .name = "codegen-probe",
    .root_module = b.createModule(.{
        .root_source_file = b.path("tests/codegen.zig"),
        // Fixed target and mode, so the verdict does not depend on -Doptimize or the host.
        .target = b.resolveTargetQuery(.{
            .cpu_arch = .x86_64,
            .os_tag = .linux,
            .cpu_model = .{ .explicit = &std.Target.x86.cpu.x86_64_v3 },
        }),
        .optimize = .fast,
    }),
});
const residue = b.addSystemCommand(&.{"python3"});
residue.addFileArg(b.path("scripts/asm_probe.py"));
residue.addFileArg(probe.getEmittedAsm());
residue.addArgs(&.{"gen_map=hand_map"}); // one gen=hand pair per abstraction
test_step.dependOn(&residue.step);
```

Leave out `expectExitCode(0)`: the default already fails the step on a nonzero exit, and `expectExitCode` captures stdout, which hides the RESIDUE diff.

## Cost dimensions

Run-time instructions are one of five. Report each one an abstraction moves.

| Dimension | Measure with |
|---|---|
| run-time instructions | the probe |
| data size | `@sizeOf`, `@alignOf` in a `comptime` assert or test |
| code size | `nm -S --size-sort` on the object, per instantiation |
| compile time | `zig build-obj` wall time with a fresh `--cache-dir`, against the version without the abstraction |
| Debug-build speed | the same benchmark built `-O Debug`: abstractions that fold in ReleaseFast stay calls in Debug |

## Lab evidence (Zig 0.17.0, ReleaseFast, `-mcpu=x86_64_v3`)

Every row is a probe in the lab (`lab/zig/probe.zig`, `probe_safe.zig`, `exist.zig`, `lswitch.zig`). Timings: i9-14900HX, one pinned core, 16 Ki f32, best of 30.

| Abstraction | Verdict against the hand version |
|---|---|
| comptime generic `Map(T, f)` | FOLDED |
| `enum(u32) { _ }` handle indexing a table | FOLDED |
| comptime interface (`comptime F: type`, `f.apply(x)`) | FOLDED |
| `packed struct(u8)` field reads | NEGATIVE (54 instructions against 59 for hand shifts and masks) |
| tagged union + exhaustive `switch` | IDENTICAL to a hand tag + `extern struct` + `switch` |
| comptime existence (`void` field) | FOLDED; a `?[]f64` field instead: RESIDUE ([comptime.md](comptime.md)) |
| runtime interface (ctx pointer + fn pointer) | RESIDUE: an indirect call per element, no inlining; 1.055 against 0.264 ns/element |
| 128-byte struct by value | callee IDENTICAL to the `*const` version; caller passes a pointer, no copy |
| labeled `switch` with `continue :state` | same shape as `while` + `switch`: one indirect jump per prong in both (the tool says RESIDUE on one setup instruction) |
| explicit `@Vector(8, f32)` sum | NEGATIVE: vector against a scalar loop; 0.034 against 0.528 ns/element with 4 accumulators |
| explicit `@Vector(8, u32)` sum | NEGATIVE |

Plain `for` loops stay scalar: an f32 map, a u32 sum and an f32 sum all compile to scalar instructions unrolled 8×, because Zig 0.16 and 0.17 ship with LLVM's loop vectorizer disabled (0.17.0 release notes). The f32 sum would stay scalar under any vectorizer without reassociation; the u32 sum and the f32 map are the ones the vectorizer would have taken. On 0.17 vector code is therefore written, not hoped for: [simd-first.md](simd-first.md). 0.18 turns the vectorizer back on (confirmed on 0.18.0-dev.349): the u32 sum and the map then vectorize, and the strict f32 sum stays scalar until it is moved into a gated fast-math function (simd-loops [zig.md](../simd-loops/zig.md) § Plain loops under 0.18). Re-run the probe after a compiler upgrade rather than trusting either version's verdict.

Debug builds pay for abstractions that fold in ReleaseFast. Same benchmark, `-O Debug`: the comptime `Map` 2.13 ns/element against 0.88 for the hand loop (a call per element), the runtime interface 2.68.

Code size and compile time per comptime instantiation are in [comptime.md](comptime.md). The compile-time figure measured here: 256 instantiations of a 3-line loop took 0.92 s of ReleaseFast `build-obj` against 0.05 s for one runtime-parameter body, about 3.4 ms each.

## Layout

```zig
const NodeId = enum(u32) { none = std.math.maxInt(u32), _ }; // sentinel instead of ?u32

comptime {
    assert(@sizeOf(?u32) == 8); // the flag pads to the payload's alignment
    assert(@sizeOf(NodeId) == 4);
    assert(@sizeOf(?*u32) == 8); // null is the free niche of a non-null pointer
    assert(@sizeOf(?[]u8) == 16);
    assert(@sizeOf(struct { a: u8, b: u64, c: u8 }) == 16); // auto layout reorders fields
    assert(@sizeOf(extern struct { a: u8, b: u64, c: u8 }) == 24); // declaration order
    assert(@sizeOf(packed struct(u8) { kind: u3, flags: u5 }) == 1);
    assert(@sizeOf(@Vector(3, u8)) == 16); // a vector is a register type, not storage
}

test "sentinel handle" {
    const id: NodeId = .none;
    try std.testing.expectEqual(std.math.maxInt(u32), @backingInt(id));
    const raw: NodeId = @fromBackingInt(7);
    try std.testing.expect(raw != .none);
}
```

Hot tables store handles as `enum(u32)` with a named sentinel, not `?u32` (half the bytes) and not `?*T` (twice the bytes, and it pins the storage).

## Call conventions

From ESPice, confirmed by the 128-byte probe above:

- **Read-only input: by value `T`.** Zig passes a large struct by hidden reference, so by value costs nothing and states immutability. The semantics stay value semantics: a callee that writes through another pointer to the caller's struct still reads the old value (checked in Debug, ReleaseSafe and ReleaseFast), because the compiler copies at the call site exactly when aliasing is possible.
- **The callee mutates: `*T`.** Large and identity matters: `*const T`. Small PODs never as `*const`.
- **Methods that only read take `self: Self`**, or `*const Self` when large.
- **Hot loops take primitive arguments** (slices, scalars) rather than `self` (TigerStyle), so the optimizer need not prove that struct fields stay in registers.

TigerStyle passes structs over 16 bytes as `*const` to catch accidental copies. In Zig 0.17 by-value does not copy in the probe, so that is a review aid, not a cost; follow the project's convention.

## Control flow

```zig
const Op = enum(u8) { inc, dbl, halt };

/// A labeled switch: each prong names its successor with `continue :state`.
fn run(code: []const Op, x0: i64) i64 {
    var pc: usize = 0;
    var x = x0;
    state: switch (code[0]) {
        .inc => {
            x +%= 1;
            pc += 1;
            continue :state code[pc];
        },
        .dbl => {
            x *%= 2;
            pc += 1;
            continue :state code[pc];
        },
        .halt => return x,
    }
}

fn parseDigit(c: u8) error{NotDigit}!u8 {
    if (c < '0' or c > '9') {
        @branchHint(.cold); // must be the first statement of the branch
        return error.NotDigit;
    }
    return c - '0';
}

test "control flow" {
    try std.testing.expectEqual(@as(i64, 4), run(&.{ .inc, .dbl, .halt }, 1));
    try std.testing.expectEqual(@as(u8, 7), try parseDigit('7'));
    try std.testing.expectError(error.NotDigit, parseDigit('x'));
}
```

Probed with a 5-op interpreter, the labeled switch and a `while` + `switch` compile to the same shape: one `jmp [table]` at the end of every prong (LLVM threads the plain loop too). Use the labeled form because the transitions are explicit in the source, not for speed. `@branchHint` is a compile error anywhere but the first statement of a function or branch.

## Asserts that remove work

In ReleaseSafe, bounds checks are residue, and an assert at the top of a loop can remove them. Probed with `u32` and `+%` (a checked `+` adds an overflow branch that blocks the unroll by itself):

```zig
fn addInto(out: []u32, a: []const u32, b: []const u32) void {
    assert(b.len >= a.len);
    assert(out.len >= a.len);
    for (0..a.len) |i| out[i] = a[i] +% b[i];
}

fn addSame(out: []u32, a: []const u32, b: []const u32) void {
    for (a, b, out) |x, y, *o| o.* = x +% y; // lengths must be equal: one check up front
}

test "asserted loops" {
    var out: [3]u32 = undefined;
    addInto(&out, &.{ 1, 2, 3 }, &.{ 10, 20, 30 });
    try std.testing.expectEqual(@as(u32, 33), out[2]);
    addSame(&out, &.{ 1, 2, 3 }, &.{ 1, 1, 1 });
    try std.testing.expectEqual(@as(u32, 4), out[2]);
}
```

- Without the asserts: two bounds checks per iteration and no unrolling.
- With them: the two compares move before the loop and the loop unrolls 8×. Split asserts (as above, which the `assertions` skill asks for) give the same loop as one `assert(b.len >= a.len and out.len >= a.len)`; the only difference is where LLVM places the frame-pointer setup, outside the loop.
- `for (a, b, out)` gives the same unrolled loop with one length-mismatch check up front, but it requires equal lengths, not "at least".
- A read-only indexed loop (`s +%= a[i] *% b[i]`) gets its check hoisted by LLVM with no assert; the assert there saves 3 static instructions. Claim nothing for it.

These are static asm results. Benchmark before claiming a speedup.

## Sources

- Gompute `tests/codegen.zig`, `tools/codegen_check.zig`, `build.zig` (`codegen_probe` → `run_codegen_check`)
- Zig 0.17.0 release notes, "loop vectorization": https://ziglang.org/download/0.17.0/release-notes.html
- ESPice `AGENTS.md`, "Call conventions (zero-cost by choice)"
- TigerBeetle `docs/TIGER_STYLE.md`, "Performance" and "Cache Invalidation"
