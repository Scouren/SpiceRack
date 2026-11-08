# Zig — realizing the principles

Companion to foundations.md: maps each DOD principle to Zig layout mechanics. Language
idioms, the 0.17 migration, comptime, memory discipline and the codegen probe live in the
`zig` skill (`migration-0.17.md`, `comptime.md`, `memory.md`, `zero-cost.md`); read those
for anything about how to write Zig rather than how to lay data out. Sections 4–5 cover
the language's intended handling of optionals and errors.

---

## 1. Data-Oriented Design in Zig

Zig is, in practice, the reference language for DOD: the techniques in Kelley's talk
were developed _on the Zig compiler_, and several of them are first-class in the std
library.

### Struct-of-Arrays: `std.MultiArrayList`

This is the native realization of "eliminate padding with SoA." It stores the fields of
a struct as separate arrays but presents a list-like API.

```zig
const Particle = struct { pos: [3]f32, vel: [3]f32, color: u32, alive: bool };

var ps: std.MultiArrayList(Particle) = .empty;
defer ps.deinit(allocator);
try ps.append(allocator, .{ .pos = .{0,0,0}, .vel = .{1,0,0}, .color = 0, .alive = true });

// A pass that only needs pos and vel gets two contiguous column slices —
// no color/alive bytes dragged into cache, autovectorizable:
const positions = ps.items(.pos);   // [][3]f32
const vels      = ps.items(.vel);   // [][3]f32
for (positions, vels) |*p, v| { p[0] += v[0]; p[1] += v[1]; p[2] += v[2]; }
```

`MultiArrayList` also reorders fields internally to minimize padding, so even the
footprint is smaller than the equivalent `[]Particle`. Use it when passes are
column-wise; keep a plain `[]Particle` (AoS) when every pass touches whole elements.

### Indices instead of pointers

A pointer is 8 bytes and pins the backing storage. An index is usually a `u32` and lets
you grow/relocate the arena freely. Make the index a distinct type so it can't be
confused with an ordinary integer:

```zig
const NodeIndex = enum(u32) { _ };          // a u32 you can't accidentally do math on
const Node = struct { first_child: NodeIndex, next_sibling: NodeIndex, tag: Tag };
var nodes: std.MultiArrayList(Node) = .empty;
// "pointer" to a node is now `NodeIndex`; the tree is positions in `nodes`.
```

### Booleans / cold fields out of band

Don't pay padding for a rare flag inside a hot struct. Keep hot fields in the main
array; move rare data to a side structure keyed by the same index (e.g. a small
`std.AutoHashMapUnmanaged(NodeIndex, Diagnostics)` consulted only on the cold path).

### Encodings instead of polymorphism: tagged unions + packed fields

```zig
const Shape = union(enum) { circle: f32, rect: struct { w: f32, h: f32 } };
// Branch on the tag at the top of a loop, or bucket elements by tag so each
// inner loop is monomorphic. No vtable, no per-element indirect call.
```

Zig's arbitrary-width integers and `packed struct` let you shrink elements to the bit:

```zig
const Cell = packed struct(u8) { kind: u3, flags: u3, dirty: bool, visited: bool };
// Exactly one byte. More elements per cache line, often free vectorization.
```

### Compile-time maps and enum tables (no runtime hashing for static data)

```zig
const keywords = std.StaticStringMap(Token).initComptime(.{
    .{ "fn", .kw_fn }, .{ "const", .kw_const }, .{ "return", .kw_return },
});
// Built at compile time — the Zig analogue of a perfect-hash table.

const counts = std.enums.EnumArray(Color, u32).initFill(0); // dense array keyed by enum
```

---

## 2. Negative-overhead abstraction in Zig

Abstraction happens at compile time, so a comptime generic leaves no runtime residue: on
Zig 0.17 a comptime `Map(T, f)` folds into the same symbol as the hand-written loop, while
a runtime ctx-pointer-plus-fn-pointer interface leaves an indirect call per element. That
is measured, not assumed — the `zig` skill's `zero-cost.md` holds the probe protocol and
the full verdict table. Plain loops are not autovectorized on 0.16 or 0.17 (section 9).

---

## 3. API design in Zig (Muratori's rules, language-enforced)

Several of Muratori's rules are _defaults_ in idiomatic Zig:

- **Caller owns memory.** Functions that allocate take an `Allocator` parameter; the
  caller decides the strategy. Arenas by lifetime and the full discipline: the `zig`
  skill's `memory.md`.
- **Transparent data.** Plain `struct`s with public fields are the norm; expose them
  unless there's an invariant. This keeps data inspectable, serializable, and
  SoA-able — Muratori's "transparent unless a reason to be opaque."
- **Granularity.** Offer the fine-grained primitive and a convenience wrapper built on
  it. Example shape: `parseAlloc(io, allocator, bytes)` (convenience) implemented on top
  of `parse(io, scratch, bytes, out)` (caller owns `out` and scratch).
- **Error sets are part of the API.** A function's error set (below) is a precise,
  checked contract of how it can fail — surface a named set rather than a catch-all when
  callers need to discriminate.

### I/O is an interface — pass `Io`, like you pass `Allocator`

Anything that performs I/O, blocks, or introduces nondeterminism takes an `Io`. There is
no global I/O and no function coloring; you choose threaded vs. evented backends at the
top via "Juicy Main":

```zig
pub fn main(init: std.process.Init) !void {
    const io = init.io;           // pass `io` down to anything doing I/O
    // args come from `init`, not a global `std.os.argv` (which was removed)
}
```

`GenericReader`/`AnyReader`/`FixedBufferStream` were removed; use `std.Io.Reader` /
`std.Io.Writer` (a fixed buffer writer is `std.Io.Writer.fixed(buf)`; a file's
reader/writer is reached via `file.reader(&buf).interface`).

---

## 4. Optionals — "absence is a normal, expected outcome"

Zig draws a hard line between _absence_ (optional) and _failure_ (error). Use an
optional `?T` when "nothing" is an ordinary, expected result — a lookup miss, an
end-of-iteration, a not-yet-set field. Absence is **not** an error and should not be
modeled as one.

```zig
fn find(haystack: []const u8, needle: u8) ?usize {
    for (haystack, 0..) |c, i| if (c == needle) return i;
    return null;                 // expected, ordinary "not found"
}
```

Idiomatic consumption:

```zig
// Unwrap-or-default:
const i = find(s, '/') orelse s.len;

// Capture if present (the standard form — no null deref possible):
if (find(s, '/')) |idx| { useIt(idx); } else { noSlash(); }

// Iterators return optionals; the loop ends when it yields null:
while (it.next()) |item| { process(item); }

// Assert-present (panics in safe builds if null) — use only when truly impossible:
const v = maybe.?;
```

Guidance: prefer `orelse` and `if (opt) |v|` capture; reserve `.?` for cases the
surrounding logic has already guaranteed. Don't reach for an error union to express
"not found."

---

## 5. Error handling — "something went wrong," as a checked value

Failures are values, via **error sets** and **error unions**. An error set is a set of
named error values; an error union `E!T` is "either an `E` or a `T`."

```zig
const ParseError = error{ Unexpected, Overflow };

// `!T` infers the error set from the body; name the set when callers must discriminate.
fn parseU32(bytes: []const u8) ParseError!u32 {
    var acc: u32 = 0;
    for (bytes) |c| {
        if (c < '0' or c > '9') return error.Unexpected;
        acc = std.math.mul(u32, acc, 10) catch return error.Overflow;
        acc += c - '0';
    }
    return acc;
}
```

Propagation and handling:

```zig
// `try` = "evaluate, and if it's an error, return it from the current function":
const n = try parseU32(field);

// `catch` = handle locally, optionally with the error captured:
const n2 = parseU32(field) catch 0;                 // substitute a default
const n3 = parseU32(field) catch |err| switch (err) {
    error.Overflow => return reportTooBig(),
    error.Unexpected => return reportBadChar(),
};
```

Cleanup on the _error_ path only — the counterpart to `defer`:

```zig
const buf = try allocator.alloc(u8, n);
errdefer allocator.free(buf);       // freed only if a later `try` in this fn fails
try fillOrFail(buf);                // on failure, errdefer runs; on success it doesn't
return buf;                         // success: caller owns `buf`
```

### Combining the two

`!?T` is a function that can _fail_ (error union) and, when it succeeds, may legitimately
return _nothing_ (optional): e.g. `fn next(self: *It) !?Token` — reading can error, and
a successful read can be end-of-stream (`null`). This is the precise, idiomatic way to
say both things at once.

### Conventions

- Optional for expected absence; error union for genuine failure. Never use an error to
  signal an ordinary "not found."
- `try` to propagate, `catch` to handle, `errdefer` to unwind partial work.
- Don't discard errors silently. If a failure is truly impossible, make that explicit
  (`catch unreachable`) rather than swallowing it.
- A function's error set is part of its contract — keep it as narrow and named as the
  callers need.

---

## 6. Memory and allocation in Zig

Every allocating function takes an `Allocator`, so matching allocator lifetime to data
lifetime (foundations.md, question 5) is enforced by the language. Arenas by lifetime,
`errdefer` pairing, complete deinit, leak tests and the 0.17 allocator names: the `zig`
skill's `memory.md`.

---

## 7. Concurrency in Zig (via `std.Io`)

`std.Thread.Pool` was removed in 0.16; concurrency lives in the `Io` interface, chosen
at the top like the allocator. Two primitives, with a crucial distinction:

- **`io.async(fn, args)` -> `Future(T)`** expresses _asynchrony_: the call is
  independent of surrounding logic. Creating it is infallible and portable, even on `Io`
  implementations with no real concurrency (it may run on the same thread).
- **`io.concurrent(...)`** expresses genuine _concurrency_: the tasks must make progress
  simultaneously (e.g. a producer and consumer that synchronize). It can fail with
  `error.ConcurrencyUnavailable` on a single-threaded backend.

```zig
// One independent task, awaited later:
var future = io.async(compute, .{input});
const result = future.await(io);

// A group of data-parallel tasks over disjoint chunks (partition, don't share):
var group: std.Io.Group = .init;
for (chunks) |chunk| try group.concurrent(io, processChunk, .{ io, chunk });
try group.await(io);
```

Backends: `std.Io.Threaded` is the usable one (a thread pool); `std.Io.Evented`
(io_uring on Linux, GCD on macOS) is experimental in 0.16 — prefer `Threaded` for
anything real. For raw threads and primitives, `std.Thread`, `std.Thread.Mutex`, and
`std.atomic.Value(T)` still exist. The async-vs-concurrent choice is the one to get
right: ask for `async` when you mean "I don't care when this runs," `concurrent` when
two tasks must run at once or they deadlock.

Partition, don't share: give each task a disjoint chunk it owns exclusively. When two
threads do write hot per-thread values, keep them on separate cache lines
(`align(std.atomic.cache_line)`) so the line doesn't ping-pong between cores (false
sharing).

---

## 8. Type-driven design in Zig

Zig has no Rust-style typestate, but it gives you sharp tools to make illegal states
unrepresentable:

- **Tagged unions over tag-plus-untyped-payload.** `union(enum)` forces every `switch`
  to handle the active field, and you cannot read the wrong variant. This is the
  "encoding instead of polymorphism" technique _and_ the correctness technique at once.
- **Distinct index/ID types** via `enum(u32) { _ }` so an index can't be confused with
  an integer or subjected to arithmetic.
- **Narrowest representation**: `u3`, `enum(u8)`, `packed struct` — fewer bit patterns
  means fewer illegal states.
- **Optionals and error unions are this principle built into the language**: `?T` makes
  absence impossible to ignore (no null deref in safe code), and `E!T` makes failure
  impossible to ignore (the result must be handled or propagated).
- **Parse, don't validate**: have a parse function return a refined struct
  (`fn parse(bytes) !Config`), and let the rest of the program take `Config` by value —
  no re-checking downstream. Push as much as possible to `comptime` when the inputs are
  known at compile time.

---

## 9. SIMD in Zig

On Zig 0.16 and 0.17 vectors are written, not hoped for: the loop vectorizer is disabled, so a plain `for` loop stays scalar. 0.18 re-enables it, and a plain loop over a contiguous SoA column then vectorizes when it is written in the measured forms (simd-loops [zig.md](../simd-loops/zig.md) § Plain loops under 0.18). That makes the layout matter more: a strided AoS field or a pointer chase is exactly what the vectorizer gives up on. Where the lanes come from (the lane axis, the `W == 1` oracle) is the `zig` skill's `simd-first.md`. Vectorizing a hot loop is the simd-loops skill's job — its [zig.md](../simd-loops/zig.md) carries the `@Vector` builtin table, worked reduce idioms, and the target-CPU gotchas.

What belongs *here* is where a vector may live, because that is a layout decision and it gets made before any loop is written.

### `@Vector` is a register type, not a storage type

Layout is the decision; `@Vector` is the loop detail. Store fields the way section 1 says — contiguous, one array per field — and materialize vectors inside the loop:

```zig
// boundary: errors, parsing, allocation
pub fn parse(arena: Allocator, input: []const u8) ![]i32 { ... }

// interior: no allocator, no errors, vectors sized to the machine
pub fn sum(xs: []const i32) i64 {
    const lanes = std.simd.suggestVectorLength(i32) orelse return scalarSum(xs);
    const V = @Vector(lanes, i32);
    var acc: V = @splat(0);
    var i: usize = 0;
    while (i + lanes <= xs.len) : (i += lanes) acc += @as(V, xs[i..][0..lanes].*);
    ...
}
```

**`@Vector` never appears in a struct field or a public signature.** It is not an array with operators — it is a distinct type with its own ABI, and the differences bite exactly where you would use it as storage:

| | `[N]T` | `@Vector(N, T)` |
|---|---|---|
| `[4]f32` | size 16, align 4 | size 16, **align 16** |
| `[3]u8` | size 3, align 1 | size **16**, align 16 |
| `[8]bool` | size 8, align 1 | size **1** (bit-packed) |

Element types are limited to bool, int, float, and pointer — no structs, no slices — so a "keep everything in SIMD" struct cannot hold the string half of its own data anyway.

### Size vectors to the machine, not to the data

`@Vector(128, i32)` is 512 bytes: sixteen `ymm` registers for one value, on a machine that has sixteen total. It compiles to a stack frame, not to registers:

```asm
sum_native:                       sum_big:                      # @Vector(128, i32)
  vpmulld ymm0, ymm1, ymm0          and     rsp, -512
  ...                               sub     rsp, 512            # spilled before it started
  ret                               vmovdqa ymm0, [rbp + 688]   # args arrived in memory
```

`std.simd.suggestVectorLength(T)` is the width, resolved at comptime against the target. A comptime `N` chosen from the data also silently caps how much data the function accepts.

Indexing a vector with a runtime index round-trips it through memory — the escape hatch back to scalar exists and is silent, which is the other reason vectors stay local to the loop that reduces them.
