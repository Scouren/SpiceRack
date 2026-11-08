# Zig 0.17 layout

Open this when you are writing the row layout, intern pool, fact table, diagnostic catalogue or a worklist walk for a Zig frontend. The file below is one unit and passes `zig test` under 0.17.0 in Debug and ReleaseFast. Copy it and grow it. Every rule number refers to SKILL.md.

How it maps to the rules:

- `Token` / `TokenList`: rule 1. 5 bytes a token as two columns.
- `Node`, `Node.Data`, `Ast.extra_data`, `Ast.addList`: rule 1. 13 bytes a node; `call` stores its argument list as `[len, args...]` in `extra_data`.
- `InternPool`: rule 2. Bytes plus end offsets; the map's key is the `StrId` and `Adapter` looks up by text without interning.
- `class`: rule 4. `std.EnumArray(...).init` requires every field, so deleting `.call = .call` fails with `error: missing struct field: call`.
- `Code`, `catalogue`, `Diag`: rule 5.
- `eval` refusing `.call` with `E0410`: rule 6.
- `eval`'s `work` stack: rule 7. The last test walks 10⁶ levels. The same tree evaluated recursively in Debug segfaulted past 10⁴ levels on a 16 MiB stack.

0.17 spellings used: `@backingInt` / `@fromBackingInt` for enum↔integer, `std.ArrayList(T)` unmanaged with `.empty` and an allocator per call, `MultiArrayList.items(.field)`, `ArrayList.pop()` returning `?T`, and `HashMapUnmanaged.getOrPutContextAdapted`. Stdlib idioms beyond these are in the `zig` skill.

```zig
//! A minimal frontend layout: token rows, node rows with `extra_data`, an
//! intern pool, a per-tag fact table, coded diagnostics and a worklist walk.
//! In: rows a parser appends. Out: an `i64`, or `null` plus `Diag` rows.

const std = @import("std");
const assert = std.debug.assert;
const Allocator = std.mem.Allocator;

pub const Token = struct {
    tag: Tag,
    /// Byte offset of the token's first byte; the end is re-lexed on demand.
    start: u32,

    pub const Tag = enum(u8) { identifier, int_literal, plus, star, l_paren, r_paren, comma, eof };
};
/// Columns `.tag` (1 B) and `.start` (4 B): 5 bytes a token, no padding.
pub const TokenList = std.MultiArrayList(Token);

/// Dense id of an interned name, in first-seen order.
pub const StrId = enum(u32) { _ };

/// Names stored once: bytes plus offsets, deduplicated through a map whose
/// key is the 4-byte id and whose hash reads the stored bytes.
pub const InternPool = struct {
    bytes: std.ArrayList(u8) = .empty,
    /// `ends[i]` is one past name i's last byte.
    ends: std.ArrayList(u32) = .empty,
    map: std.HashMapUnmanaged(StrId, void, Context, std.hash_map.default_max_load_percentage) = .empty,

    const Context = struct {
        pool: *const InternPool,
        pub fn hash(c: Context, id: StrId) u64 {
            return std.hash_map.hashString(c.pool.get(id));
        }
        pub fn eql(_: Context, a: StrId, b: StrId) bool {
            return a == b;
        }
    };
    const Adapter = struct {
        pool: *const InternPool,
        pub fn hash(_: Adapter, s: []const u8) u64 {
            return std.hash_map.hashString(s);
        }
        pub fn eql(a: Adapter, s: []const u8, id: StrId) bool {
            return std.mem.eql(u8, s, a.pool.get(id));
        }
    };

    pub fn deinit(p: *InternPool, gpa: Allocator) void {
        p.bytes.deinit(gpa);
        p.ends.deinit(gpa);
        p.map.deinit(gpa);
    }

    /// Returns the id of `s`, adding it on first sight. On OOM the pool is unchanged.
    pub fn intern(p: *InternPool, gpa: Allocator, s: []const u8) Allocator.Error!StrId {
        try p.bytes.ensureUnusedCapacity(gpa, s.len);
        try p.ends.ensureUnusedCapacity(gpa, 1);
        const gop = try p.map.getOrPutContextAdapted(gpa, s, Adapter{ .pool = p }, .{ .pool = p });
        if (gop.found_existing) return gop.key_ptr.*;
        const id: StrId = @fromBackingInt(@intCast(p.ends.items.len));
        p.bytes.appendSliceAssumeCapacity(s);
        p.ends.appendAssumeCapacity(@intCast(p.bytes.items.len));
        gop.key_ptr.* = id;
        return id;
    }

    pub fn get(p: *const InternPool, id: StrId) []const u8 {
        const i = @backingInt(id);
        const start = if (i == 0) 0 else p.ends.items[i - 1];
        return p.bytes.items[start..p.ends.items[i]];
    }
};

pub const Node = struct {
    tag: Tag,
    main_token: u32,
    data: Data,

    pub const Index = enum(u32) { _ };
    /// Two u32 words; each tag's doc says how to read them.
    pub const Data = struct { lhs: u32, rhs: u32 };

    pub const Tag = enum(u8) {
        /// `lhs` = `StrId`.
        identifier,
        /// `lhs` = the value (literals past u32 would go to a side table).
        int_literal,
        /// `lhs`, `rhs` = operand `Index`.
        add,
        mul,
        /// `lhs` = callee `StrId`, `rhs` = offset in `extra_data` of `[len, arg0, arg1, ...]`.
        call,
    };

    comptime {
        assert(@sizeOf(Tag) == 1);
        assert(@sizeOf(Data) == 8);
    }
};

/// What a walk needs to know about a tag, one row per tag. `init` takes every
/// field, so a new tag does not compile until its row exists.
const Class = enum { leaf, binary, call };
const class = std.EnumArray(Node.Tag, Class).init(.{
    .identifier = .leaf,
    .int_literal = .leaf,
    .add = .binary,
    .mul = .binary,
    .call = .call,
});

/// Stable codes: never renumbered or reused. The first digit pair is the stage.
pub const Code = enum(u16) { E0201, E0410 };
const Info = struct { title: []const u8, clause: []const u8 };
pub const catalogue = std.EnumArray(Code, Info).init(.{
    .E0201 = .{ .title = "expected an expression", .clause = "A.8.3" },
    .E0410 = .{ .title = "function call is parsed but not yet evaluated", .clause = "4.7" },
});
/// One diagnostic row. Text is rendered later from `catalogue` and the token.
pub const Diag = struct { code: Code, token: u32 };

pub const Ast = struct {
    nodes: std.MultiArrayList(Node) = .empty,
    extra_data: std.ArrayList(u32) = .empty,

    pub fn deinit(a: *Ast, gpa: Allocator) void {
        a.nodes.deinit(gpa);
        a.extra_data.deinit(gpa);
    }

    pub fn add(a: *Ast, gpa: Allocator, node: Node) Allocator.Error!Node.Index {
        try a.nodes.append(gpa, node);
        return @fromBackingInt(@intCast(a.nodes.len - 1));
    }

    /// Appends `[len, items...]` and returns the offset of `len`.
    pub fn addList(a: *Ast, gpa: Allocator, items: []const Node.Index) Allocator.Error!u32 {
        const at: u32 = @intCast(a.extra_data.items.len);
        try a.extra_data.ensureUnusedCapacity(gpa, items.len + 1);
        a.extra_data.appendAssumeCapacity(@intCast(items.len));
        for (items) |i| a.extra_data.appendAssumeCapacity(@backingInt(i));
        return at;
    }
};

/// Post-order evaluation on an explicit stack, so nesting depth costs heap,
/// not native stack. `env` is indexed by `StrId`. A call is refused with a
/// coded diagnostic rather than evaluated to a guess.
pub fn eval(gpa: Allocator, ast: *const Ast, root: Node.Index, env: []const i64, diags: *std.ArrayList(Diag)) Allocator.Error!?i64 {
    const tags = ast.nodes.items(.tag);
    const datas = ast.nodes.items(.data);
    const Frame = struct { node: Node.Index, operands_done: bool };
    var work: std.ArrayList(Frame) = .empty;
    defer work.deinit(gpa);
    var values: std.ArrayList(i64) = .empty;
    defer values.deinit(gpa);

    try work.append(gpa, .{ .node = root, .operands_done = false });
    while (work.pop()) |f| {
        const i = @backingInt(f.node);
        const d = datas[i];
        switch (class.get(tags[i])) {
            .leaf => try values.append(gpa, switch (tags[i]) {
                .identifier => env[d.lhs],
                .int_literal => d.lhs,
                .add, .mul, .call => unreachable, // class says leaf
            }),
            .binary => if (!f.operands_done) {
                // Pushed in reverse so lhs is evaluated first.
                try work.append(gpa, .{ .node = f.node, .operands_done = true });
                try work.append(gpa, .{ .node = @fromBackingInt(d.rhs), .operands_done = false });
                try work.append(gpa, .{ .node = @fromBackingInt(d.lhs), .operands_done = false });
            } else {
                const rhs = values.pop().?;
                const lhs = values.pop().?;
                try values.append(gpa, switch (tags[i]) {
                    .add => lhs +% rhs,
                    .mul => lhs *% rhs,
                    .identifier, .int_literal, .call => unreachable, // class says binary
                });
            },
            .call => {
                try diags.append(gpa, .{ .code = .E0410, .token = ast.nodes.items(.main_token)[i] });
                return null;
            },
        }
    }
    assert(values.items.len == 1);
    return values.items[0];
}

test "row sizes" {
    // Bytes per row of a MultiArrayList: the sum of its column widths.
    try std.testing.expectEqual(5, @sizeOf(Token.Tag) + @sizeOf(u32));
    try std.testing.expectEqual(13, @sizeOf(Node.Tag) + @sizeOf(u32) + @sizeOf(Node.Data));
    // The same holds for the stdlib's own Zig AST tokens.
    try std.testing.expectEqual(1, @sizeOf(std.zig.Token.Tag));
}

test "intern returns one id per text" {
    const gpa = std.testing.allocator;
    var pool: InternPool = .{};
    defer pool.deinit(gpa);
    const a = try pool.intern(gpa, "alpha");
    const b = try pool.intern(gpa, "beta");
    try std.testing.expectEqual(a, try pool.intern(gpa, "alpha"));
    try std.testing.expect(a != b);
    try std.testing.expectEqualStrings("beta", pool.get(b));
}

test "eval folds, and refuses a call with E0410" {
    const gpa = std.testing.allocator;
    var pool: InternPool = .{};
    defer pool.deinit(gpa);
    var ast: Ast = .{};
    defer ast.deinit(gpa);
    var diags: std.ArrayList(Diag) = .empty;
    defer diags.deinit(gpa);

    // x * 3 + 4, with x = 5
    const x = try pool.intern(gpa, "x");
    const n_x = try ast.add(gpa, .{ .tag = .identifier, .main_token = 0, .data = .{ .lhs = @backingInt(x), .rhs = 0 } });
    const n_3 = try ast.add(gpa, .{ .tag = .int_literal, .main_token = 2, .data = .{ .lhs = 3, .rhs = 0 } });
    const n_mul = try ast.add(gpa, .{ .tag = .mul, .main_token = 1, .data = .{ .lhs = @backingInt(n_x), .rhs = @backingInt(n_3) } });
    const n_4 = try ast.add(gpa, .{ .tag = .int_literal, .main_token = 4, .data = .{ .lhs = 4, .rhs = 0 } });
    const n_add = try ast.add(gpa, .{ .tag = .add, .main_token = 3, .data = .{ .lhs = @backingInt(n_mul), .rhs = @backingInt(n_4) } });
    try std.testing.expectEqual(19, (try eval(gpa, &ast, n_add, &.{5}, &diags)).?);

    // f(x, 3): parsed in full, refused by the evaluator with a named code.
    const f = try pool.intern(gpa, "f");
    const args = try ast.addList(gpa, &.{ n_x, n_3 });
    const n_call = try ast.add(gpa, .{ .tag = .call, .main_token = 6, .data = .{ .lhs = @backingInt(f), .rhs = args } });
    try std.testing.expectEqual(null, try eval(gpa, &ast, n_call, &.{ 5, 0 }, &diags));
    try std.testing.expectEqual(Code.E0410, diags.items[0].code);
    try std.testing.expectEqual(2, ast.extra_data.items[args]);
    try std.testing.expectEqualStrings("4.7", catalogue.get(diags.items[0].code).clause);
}

test "a million levels of nesting walk on the heap" {
    const gpa = std.testing.allocator;
    var ast: Ast = .{};
    defer ast.deinit(gpa);
    var diags: std.ArrayList(Diag) = .empty;
    defer diags.deinit(gpa);
    // 1 + (1 + (1 + ...)), built innermost first.
    var top = try ast.add(gpa, .{ .tag = .int_literal, .main_token = 0, .data = .{ .lhs = 1, .rhs = 0 } });
    for (0..1_000_000) |_| {
        const one = try ast.add(gpa, .{ .tag = .int_literal, .main_token = 0, .data = .{ .lhs = 1, .rhs = 0 } });
        top = try ast.add(gpa, .{ .tag = .add, .main_token = 0, .data = .{ .lhs = @backingInt(one), .rhs = @backingInt(top) } });
    }
    try std.testing.expectEqual(1_000_001, (try eval(gpa, &ast, top, &.{}, &diags)).?);
}
```

## Where the real ones are

- `lib/std/zig/Ast.zig`: `Node.Data` is a bare union of two-word shapes (`node_and_node`, `extra_range`, ...), asserted 8 bytes outside safety builds. `extraData(index, T)` decodes a struct of `u32` fields from `extra_data` by field order.
- Zig `src/InternPool.zig`: `Item{tag: Tag, data: u32}`, with `Tag = enum(u8)` documenting how each tag reads `data` and its extra words. Types and values are interned, so index equality is value equality within a type.
- VerA `lib/frontend/ast.zig` (interner keyed by id, ids in append order) and `lib/frontend/ast_expr.zig` (`ExprStore`: `MultiArrayList(Node)` plus a `[len, ...]` pool and `reals`/`ints` side tables).
