//! Residue probe. Copy to `tests/codegen.zig` and replace the example pair.
//!
//! Protocol: every abstraction under test gets two `export fn`s with the same C-ABI signature,
//! `gen_<name>` (the code written through the abstraction) and `hand_<name>` (the loop you would
//! write without it). Nothing links or runs this object: the build emits its assembly and
//! `scripts/asm_probe.py` compares each pair. FOLDED, IDENTICAL, EQUIVALENT and NEGATIVE pass;
//! RESIDUE or a missing symbol fails `zig build test`.
//!
//! Rules that keep the probe honest:
//! - Each export calls the abstraction directly. A shared helper puts a call between the export
//!   and the loop, which is the inlining under test.
//! - Pass runtime values (pointer + length, scalars) as parameters, so nothing constant-folds.
//! - A `noinline` helper shows up as `jmp helper`; compare the helper by its own name too
//!   (`helper=hand_x`), or the verdict sees only the jump.
//!
//! Standalone: zig build-obj -O ReleaseFast -mcpu=x86_64_v3 -femit-asm=probe.s -fno-emit-bin probe.zig
//!             python3 asm_probe.py probe.s gen_map=hand_map

const std = @import("std");

fn Map(comptime T: type, comptime f: fn (T) T) type {
    return struct {
        fn run(xs: []T) void {
            for (xs) |*x| x.* = f(x.*);
        }
    };
}

fn double(x: f32) f32 {
    return x * 2;
}

export fn gen_map(ptr: [*]f32, len: usize) void {
    Map(f32, double).run(ptr[0..len]);
}

export fn hand_map(ptr: [*]f32, len: usize) void {
    for (ptr[0..len]) |*x| x.* = x.* * 2;
}
