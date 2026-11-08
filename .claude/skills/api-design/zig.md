# Zig API Patterns

How the five characteristics land in Zig. Reference for [`SKILL.md`](SKILL.md).

Zig's defaults already sit on the right side of three axes: no hidden allocation, no hidden
control flow, no vtables unless you build one. The work is mostly in granularity and retention.

## The allocator parameter is the decoupling

Every function that allocates takes `std.mem.Allocator`. This is the built-in answer to
"allocation coupled to initialization" — the caller picks arena, fixed buffer, page, or their own.

```zig
pub fn parse(allocator: std.mem.Allocator, input: []const u8) !Ast {
    var nodes = std.ArrayList(Node).init(allocator);
    errdefer nodes.deinit();
    // ...
    return Ast{ .nodes = try nodes.toOwnedSlice() };
}
```

But an allocator parameter is not the finest tier. It still means you decide the sizes and the
count. Offer the query-then-fill pair below it:

```zig
// Coarse: we allocate.
pub fn parse(allocator: std.mem.Allocator, input: []const u8) !Ast;

// Fine: caller sizes and owns the memory; we only write into it.
pub fn parseNodeCount(input: []const u8) usize;
pub fn parseInto(nodes: []Node, input: []const u8) !Ast;
```

That third function is what a caller reaches for when they want the AST inside a buffer they
mapped, or inside a frame arena that resets.

## Comptime tiering, not comptime cleverness

`comptime` generates the specialized coarse tier over a fine one, so the convenience costs nothing
at runtime and stays trivially replaceable.

```zig
pub fn RingBuffer(comptime T: type, comptime capacity: usize) type {
    return struct {
        const Self = @This();

        buf: [capacity]T = undefined,
        head: usize = 0,
        len: usize = 0,

        pub fn push(self: *Self, item: T) void {
            self.buf[(self.head + self.len) % capacity] = item;
            if (self.len < capacity) {
                self.len += 1;
            } else {
                self.head = (self.head + 1) % capacity;
            }
        }

        pub fn get(self: *const Self, index: usize) ?T {
            if (index >= self.len) return null;
            return self.buf[(self.head + index) % capacity];
        }
    };
}
```

Resist generating types the caller then has to hold. If a comptime type appears in a signature
where a slice would do, it is coupling — see below.

## Slices are the universal boundary type

The strongest anti-coupling move in Zig is that almost everything is expressible as `[]u8`,
`[]const T`, or a plain extern struct.

```zig
// BAD: the caller must construct our type to ask us anything, and we own the file I/O.
pub fn loadTexture(path: []const u8) !Texture;

// GOOD: caller reads bytes however they like; we only interpret memory.
pub fn textureSizeFor(header: *const TextureHeader) usize;
pub fn textureFromMemory(mem: []u8) *Texture;
pub const TextureHeader = extern struct { width: u32, height: u32, format: Format };
```

`extern struct` on plain-data types is deliberate transparency: the layout is guaranteed, so the
caller can read it from their own packed stream, mmap it, or ship it to the GPU without going
through you. Data with no reason to be opaque should be transparent.

## Error sets encode what can fail, in the signature

```zig
const InitError = error{ DeviceNotFound, OutOfMemory, InvalidConfig };

pub fn init(allocator: std.mem.Allocator, config: Config) InitError!Device {
    const raw = openDevice(config.id) orelse return error.DeviceNotFound;
    errdefer closeDevice(raw);
    const mem = try allocator.alloc(u8, config.buf_size);
    return Device{ .handle = raw, .buffer = mem };
}
```

Name the set explicitly on public functions rather than inferring with `!`. An inferred error set
is an unversioned contract: it silently grows when you add a `try`, and every caller's exhaustive
switch breaks. Explicit sets are the Zig form of "don't hide what the caller must handle."

## `defer` / `errdefer` beat destructors

Cleanup is visible at the point of acquisition, and there is no hidden code running at a scope
exit you did not write.

```zig
pub fn processFile(allocator: std.mem.Allocator, path: []const u8) !void {
    const file = try std.fs.cwd().openFile(path, .{});
    defer file.close();

    const buf = try allocator.alloc(u8, 4096);
    errdefer allocator.free(buf);
    // ...
}
```

This is why a Zig API should return resources rather than register them. A `deinit` the caller
calls is immediate; a resource the library tracks and frees for you is retention.

## The request union: I/O without callbacks

Zig has no closures with hidden environments, which makes the coroutine-style API the natural
shape rather than a contortion. The library never calls you; it tells you what it needs and stops.

```zig
pub const Request = union(enum) {
    file_mapping: struct { offset: usize, len: usize },
    toc_memory: struct { size: usize, alignment: usize },
    output_memory: struct { size: usize },
    done,
};

pub const Unzip = struct {
    // opaque progress state; the caller never inspects it
    pub fn init(total_size: usize) Unzip { ... }
    pub fn initWithToc(toc: []u8) Unzip { ... }
};

/// Returns false when finished. Fill `req`'s answer via the supply* calls and call again.
pub fn unzip(self: *Unzip, req: *Request) bool { ... }
pub fn supplyMemory(self: *Unzip, mem: []u8) void { ... }
```

Driven by the caller:

```zig
var req: Request = undefined;
while (unzip(&state, &req)) {
    switch (req) {
        .file_mapping => |m| unzip.supplyMemory(&state, try mapFile(f, m.offset, m.len)),
        .toc_memory => |t| unzip.supplyMemory(&state, try arena.alignedAlloc(u8, t.alignment, t.size)),
        .output_memory => |o| unzip.supplyMemory(&state, try arena.alloc(u8, o.size)),
        .done => break,
    }
}
```

What this buys, all without touching the library: the caller chooses mmap or read, chooses every
allocator, can serialize the TOC to disk and reload it with `initWithToc`, and can extract in
parallel by handing one shared TOC to N independent `Unzip` states. The library only ever knows
about memory blocks.

The streaming form of the same idea, for parsers:

```zig
// BAD: builds a whole DOM whether you wanted one or not, and owns the allocation.
pub fn jsonLoad(allocator: std.mem.Allocator, path: []const u8) !Value;

// GOOD: tokenizer over caller-owned bytes. jsonLoad is then written on top of it.
pub const Token = struct { type: TokenType, str: []const u8 };
pub fn jsonRead(js: *Json, tok: *Token) bool;
```

Both tiers ship. The mistake is shipping only the first.

## Function pointers are the thing to avoid

`*const fn (...) void` in a public signature gives up flow control and couples the caller's
lifetime and context to your call stack. Zig gives you no closure to smuggle state through, so it
turns into a `*anyopaque` context pointer, which is worse.

Before adding one, check whether the request union or a polling call expresses it:

```zig
// BAD: we own the loop and the caller's scope.
pub fn readAsync(fd: posix.fd_t, cb: *const fn (ctx: *anyopaque, data: []u8) void, ctx: *anyopaque) void;

// GOOD: caller decides when to ask and what to do with the answer.
pub const ReadResult = union(enum) { ready: usize, pending, closed };
pub fn pollRead(fd: posix.fd_t, buf: []u8) ReadResult;
```

The legitimate exception is a genuine per-instance policy the library must consult on a hot path
(an allocator vtable, a logging sink). Even then, provide a default and a non-callback tier.
