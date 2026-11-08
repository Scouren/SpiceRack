# Porting 0.16 code to 0.17.0

Every row was compiled with Zig 0.17.0. **Removed** means the old form is a compile error; **deprecated** means it still compiles in 0.17.0 and will not in a later release, so port it anyway.

| 0.16 | 0.17.0 | Status |
|---|---|---|
| `@intFromEnum(e)` / `@enumFromInt(i)` | `@backingInt(e)` / `@fromBackingInt(i)` (result type inferred; also packed structs with an explicit backing int, and tagged unions) | deprecated; `zig fmt` rewrites them |
| `[1]T{v} ** n` | `const a: [n]T = @splat(v);` | removed |
| `std.heap.DebugAllocator(.{})` / `GeneralPurposeAllocator` | `std.heap.SafeAllocator.init(std.heap.page_allocator, .{})`; `deinit()` returns the leak count (`usize`) | deprecated |
| `StackFallbackAllocator(N)` | `std.heap.BufferFirstAllocator.init(&buf, fallback)` | replaced |
| `std.fmt.allocPrint(a, fmt, args)` | `a.print(fmt, args)`; `a.printSentinel(fmt, args, 0)` | deprecated |
| `list.getLast()` / `getLastOrNull()` | `list.last().?` / `list.last()`; `list.lastPtr()` gives `?*T` | deprecated |
| `@typeInfo(T).@"struct".fields` (array of `Field`) | `.field_names`, `.field_types`, `.field_attrs` (struct of arrays); `@"fn".param_types` likewise | removed |
| `@hasDecl(T, "x")` true for private decls in the same file | true only for `pub` decls, from any file | behaviour change |
| `errdefer \|err\| ...` | move the body into a caller: `inner() catch \|err\| { ...; return err; }` | removed |
| `std.meta.Int(.unsigned, n)` | `@Int(.unsigned, n)` (e.g. a lane mask: `const m: @Int(.unsigned, W) = @bitCast(cmp);`) | removed |
| `void{}` | `{}` | removed |
| `i0` | `u0` | removed |
| `@bitCast(extern_struct)` | `@as(*align(1) const U, @ptrCast(&s)).*`, or an `extern union` | removed |
| `std.bit_set.IntegerBitSet(n)`, `initEmpty()` | `std.bit_set.Integer(n)`, `.empty` / `.full` | renamed |
| `std.builtin.OptimizeMode`, `.ReleaseFast`, `builtin.mode == .ReleaseSafe` | `std.lang.Optimize`, tags `.debug .safe .fast .small` | deprecated; old tag names removed after 0.18 |
| `@import("builtin").cpu` | `@import("builtin").target.cpu` | deprecated |
| `if (b.args) \|args\| run.addArgs(args);` | `run.addPassthruArgs();` | new API; that it keeps the configure cache pure is from the release notes (unverified here) |

New in 0.17 and used across this skill: `@divCeil(a, b)`; dereferencing a slice whose length is comptime-known (`const a: [3]u16 = s.*;`).

Find what is left to port:

```sh
rg -n '@intFromEnum|@enumFromInt|[\]}]\s*\*\*|DebugAllocator|GeneralPurposeAllocator|allocPrint|getLast|errdefer \||\.fields\b|void\{\}|\bi0\b|IntegerBitSet|meta\.Int|OptimizeMode|Release(Fast|Safe|Small)\b|b\.args' --glob '*.zig'
```

Each new form, compiled:

```zig
const Status = enum(u8) { ok = 0, err = 1 };
const Flags = packed struct(u8) { a: bool, b: bool, rest: u6 };

test "conversions" {
    const raw: u8 = @backingInt(Status.err);
    const back: Status = @fromBackingInt(raw);
    try std.testing.expectEqual(Status.err, back);
    try std.testing.expectEqual(@as(u8, 0b1101), @backingInt(Flags{ .a = true, .b = false, .rest = 3 }));
    const zeros: [1024]u8 = @splat(0);
    try std.testing.expectEqual(@as(u8, 0), zeros[1023]);
    try std.testing.expectEqual(3, @divCeil(17, 8));
}

test "allocators and lists" {
    var safe: std.heap.SafeAllocator = .init(std.heap.page_allocator, .{});
    defer assert(safe.deinit() == 0);
    const gpa = safe.allocator();

    const s = try gpa.print("{s}={d}", .{ "k", 7 });
    defer gpa.free(s);

    var list: std.ArrayList(u32) = .empty;
    defer list.deinit(gpa);
    try list.append(gpa, 42);
    try std.testing.expectEqual(@as(u32, 42), list.last().?);
}

const P = struct { x: f32, y: f32, id: u32 };

test "reflection is struct-of-arrays" {
    const info = @typeInfo(P).@"struct";
    try std.testing.expectEqualStrings("id", info.field_names[2]);
    try std.testing.expectEqual(u32, info.field_types[2]);
}

const TwoBytes = extern struct { b0: u8, b1: u8 };

test "pun an extern struct" {
    const bytes: TwoBytes = .{ .b0 = 0x12, .b1 = 0xAB };
    const word: *align(1) const u16 = @ptrCast(&bytes);
    try std.testing.expectEqual(@as(u16, 0xAB12), word.*); // little-endian host
}

fn work(gpa: Allocator) !void {
    const buf = try gpa.alloc(u8, 16);
    defer gpa.free(buf);
    return error.Failed;
}

/// Replaces `errdefer |err| log(err)` inside `work`.
fn workLogged(gpa: Allocator) !void {
    work(gpa) catch |err| {
        std.log.debug("work failed: {t}", .{err});
        return err;
    };
}

test "errdefer capture moved to the caller" {
    try std.testing.expectError(error.Failed, workLogged(std.testing.allocator));
}
```

Done when the `rg` line above prints nothing and `zig build test` passes on 0.17.0.

## Sources

- Zig 0.17.0 release notes, https://ziglang.org/download/0.17.0/release-notes.html
- zigcc-skills `zig-0-17/SKILL.md` (porting table; every row re-checked here against the 0.17.0 compiler and `lib/std`)
