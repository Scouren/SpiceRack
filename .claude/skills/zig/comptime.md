# comptime: interfaces, existence, instantiation cost

Verdicts below come from `asm_probe.py` on Zig 0.17.0, `-O ReleaseFast -mcpu=x86_64_v3`; the protocol is in [zero-cost.md](zero-cost.md).

## Interfaces: comptime by default

A **comptime interface** is a type parameter plus the decls the body calls on it. It monomorphizes: probed against the hand loop it is **FOLDED** (LLVM merged both into one symbol).

```zig
const Scale = struct {
    k: f32,
    pub fn apply(self: Scale, x: f32) f32 {
        return x * self.k;
    }
};

/// `F` is any type with `pub fn apply(F, f32) f32`.
fn mapWith(comptime F: type, f: F, xs: []f32) void {
    if (!@hasDecl(F, "apply")) @compileError(@typeName(F) ++ " has no pub fn apply");
    for (xs) |*x| x.* = f.apply(x.*);
}

test mapWith {
    var xs = [_]f32{ 1, 2, 3 };
    mapWith(Scale, .{ .k = 2 }, &xs);
    try std.testing.expectEqual(@as(f32, 6), xs[2]);
}
```

`@hasDecl` sees only `pub` decls in 0.17, from every file. A method the interface requires must be `pub`, or the check rejects a type whose call would have compiled.

A **runtime interface** is a context pointer plus a function pointer (`std.mem.Allocator`, `std.Io`, a hand-written vtable). Probed: **RESIDUE**, an indirect `call` per element, and the callee's body cannot inline into the loop. Timed on an i9-14900HX, 16 Ki f32, ReleaseFast: 1.055 ns/element against 0.264 for the comptime and hand versions (4.0×). Keep a runtime interface where the choice is made at run time *and* the call count is per batch, not per element: an allocator call per allocation, ESPice's device-batch vtable at one indirect call per device type per evaluation.

```zig
const Iface = struct {
    ctx: *const anyopaque,
    apply: *const fn (ctx: *const anyopaque, x: f32) f32,
};

fn mapDyn(i: Iface, xs: []f32) void {
    for (xs) |*x| x.* = i.apply(i.ctx, x.*); // one indirect call per element
}

fn scaleDyn(ctx: *const anyopaque, x: f32) f32 {
    const s: *const Scale = @ptrCast(@alignCast(ctx));
    return x * s.k;
}

const Scale = struct { k: f32 };

test mapDyn {
    const s: Scale = .{ .k = 2 };
    var xs = [_]f32{ 1, 2, 3 };
    mapDyn(.{ .ctx = &s, .apply = &scaleDyn }, &xs);
    try std.testing.expectEqual(@as(f32, 6), xs[2]);
}
```

## Existence: comptime fields over optionals on hot paths

A field that some configurations never have is a comptime **existence** switch, `if (has_x) T else void`. The `void` arm takes 0 bytes and the code that touches it disappears.

```zig
fn Batch(comptime has_grad: bool) type {
    return struct {
        x: []f64,
        grad: if (has_grad) []f64 else void,

        fn step(b: @This(), h: f64) void {
            for (b.x, 0..) |*x, i| {
                x.* += h;
                if (has_grad) b.grad[i] += 1;
            }
        }
    };
}

test Batch {
    try std.testing.expectEqual(@as(usize, 16), @sizeOf(Batch(false)));
    try std.testing.expectEqual(@as(usize, 32), @sizeOf(Batch(true)));
    try std.testing.expectEqual(@as(usize, 16), @sizeOf(?[]f64)); // what an optional field costs instead

    var xs = [_]f64{ 1, 2 };
    const b: Batch(false) = .{ .x = &xs, .grad = {} };
    b.step(0.5);
    try std.testing.expectEqual(@as(f64, 2.5), xs[1]);
}
```

Probed against the plain loop: `Batch(false).step` is **FOLDED**. The same loop over a `grad: ?[]f64` field is **RESIDUE**: in ReleaseFast LLVM unswitches it into two full loop copies (85 instructions against 42), and in ReleaseSafe it keeps one copy with a bounds-check panic (57 instructions, 1 panic). Use an optional when absence is decided per value at run time; use existence when it is decided per configuration.

## Instantiation cost

Every distinct comptime argument is a new copy of the function. That copy is free at run time and paid in code size and compile time.

- Measured here: one loop instantiated for 16 comptime constants is 16 bodies, 4228 B of `.text` (about 264 B each); the same loop taking the constant at run time is one 232 B body.

Make a value comptime when it changes a type, a layout or an array length, or when a probe shows the folded constant buys instructions (a shift for a power-of-two divide). Pass everything else at run time.

## Other comptime idioms

```zig
const Shape = union(enum) { circle: f32, square: f32, rect: struct { w: f32, h: f32 } };

fn area(s: Shape) f32 {
    return switch (s) {
        // One prong body, instantiated per tag; `tag` is comptime-known in each copy.
        inline .circle, .square => |v, tag| if (tag == .circle) 3.0 * v * v else v * v,
        .rect => |r| r.w * r.h,
    };
}

const Particle = struct { x: f32, y: f32, id: u32 };

/// Reflection is struct-of-arrays in 0.17: zip the parallel slices.
fn countFloatFields(comptime T: type) usize {
    const info = @typeInfo(T).@"struct";
    var n: usize = 0;
    for (info.field_types) |F| n += @intFromBool(F == f32);
    return n;
}

const Pool = struct {
    used: u32,
    cap: u32,

    // Decl literals: `.empty` and `.withCapacity(n)` at the use site.
    pub const empty: Pool = .{ .used = 0, .cap = 0 };
    pub fn withCapacity(cap: u32) Pool {
        return .{ .used = 0, .cap = cap };
    }
};

const Keyword = enum { kw_fn, kw_const, kw_return };
const keywords: std.StaticStringMap(Keyword) = .initComptime(.{
    .{ "fn", .kw_fn }, .{ "const", .kw_const }, .{ "return", .kw_return },
});

const Color = enum { red, green, blue };

comptime {
    assert(@sizeOf(Particle) == 12); // assert layout facts the code relies on
    assert(@FieldType(Particle, "id") == u32);
}

test "comptime idioms" {
    try std.testing.expectEqual(@as(f32, 4), area(.{ .square = 2 }));
    try std.testing.expectEqual(@as(usize, 2), comptime countFloatFields(Particle));
    const p: Pool = .withCapacity(8);
    try std.testing.expectEqual(@as(u32, 8), p.cap);
    const q: Pool = .empty;
    try std.testing.expectEqual(@as(u32, 0), q.cap);
    try std.testing.expectEqual(Keyword.kw_const, keywords.get("const").?);
    var counts: std.enums.EnumArray(Color, u32) = .initFill(0);
    counts.getPtr(.green).* += 1;
    try std.testing.expectEqual(@as(u32, 1), counts.get(.green));
}
```

`std.StaticStringMap` is built at compile time: use it for every fixed string set (keywords, card names, CLI verbs). `std.enums.EnumArray(E, V)` is a dense array indexed by an enum, with no hashing.

## Sources

- ESPice `AGENTS.md`, "Call conventions" and "Zig idioms, mandatory"
