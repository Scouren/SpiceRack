# Memory: leak-free by construction

Take the highest rung that fits the lifetime. Higher rungs prevent leaks by construction; the last rung prevents them by discipline, and only tests catch a lapse.

1. **Take what the runtime gives you.** `main(init: std.process.Init)` receives `init.gpa` and `init.arena` (a `*std.heap.ArenaAllocator`, freed at exit).
2. **No allocation after startup** (TigerStyle). Size every table from explicit limits at init; after that nothing is freed, so nothing leaks.
3. **An arena per lifetime.** Allocations that die together share one arena, freed in one line at the phase boundary.
4. **Owned memory under the pairing rules** below, for objects whose lifetimes genuinely differ.

## The allocator is a parameter

Every function that allocates takes an `Allocator` parameter, and its name says the lifetime: `gpa` (free each allocation), `arena` / `scratch` / `result_arena` (freed by the owner of the arena). No global allocator, no allocator hidden in a context struct that the function does not otherwise need. Containers are unmanaged: `std.ArrayList(T)` starts at `.empty` and takes the allocator on every growing call and on `deinit`.

## Arenas by lifetime

Name each arena for the phase it lives through, and give each phase its own: parse scratch (dies once the input is prepared), a session arena (lives as long as the prepared data), a per-query arena (reset between queries).

```zig
const Prepared = struct { names: []const []const u8 };

/// `scratch` dies when this returns; everything in the result lives in `session`.
fn prepare(session: Allocator, scratch: Allocator, input: []const u8) !Prepared {
    var tokens: std.ArrayList([]const u8) = .empty; // scratch: never freed one by one
    var it = std.mem.tokenizeScalar(u8, input, ' ');
    while (it.next()) |tok| try tokens.append(scratch, tok);

    const names = try session.alloc([]const u8, tokens.items.len);
    for (names, tokens.items) |*name, tok| name.* = try session.dupe(u8, tok);
    return .{ .names = names };
}

test prepare {
    var session_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer session_state.deinit();
    var scratch_state = std.heap.ArenaAllocator.init(std.testing.allocator);
    defer scratch_state.deinit();

    const p = try prepare(session_state.allocator(), scratch_state.allocator(), "r1 c1 v1");
    _ = scratch_state.reset(.free_all); // the scratch phase is over; `p` is still valid
    try std.testing.expectEqualStrings("c1", p.names[1]);
}
```

An arena that outlives its phase grows without bound in a long-running loop. Reset it at the loop boundary (`reset(.retain_capacity)`), or move the long-lived objects to rung 4.

## Pairing rules for owned memory

- **One owner per allocation.** One name holds the obligation to free.
- **Pair at the allocation site.** The next line is `defer free` (the owner stays here) or `errdefer free` (ownership moves out on success). Group each allocation with its pair by blank lines so review can match them.
- **`errdefer` across every fallible gap.** Any owned value alive across a `try` has an `errdefer` until ownership moves. 0.17 removed the error capture (`errdefer |err|`); to log the error, catch it in the caller ([migration-0.17.md](migration-0.17.md)).
- **Transfer is named.** A function returning owned memory says so in its doc comment and follows std's verbs: `dupe`, `toOwnedSlice`, `create` transfer; slices and `getPtr` borrow.
- **Free with the allocator that allocated.** Store it or take it in `deinit(self, gpa)`, as std containers do.
- **Unconditional obligations.** Every path through a function frees the same set. An `if (cond) free(p)` makes ownership a run-time value; restructure until it is not.

## deinit is complete

A type with any owning field has a `deinit` that frees every owning field, elements before containers. A borrowed field is named or documented as borrowed and never freed.

```zig
const Bag = struct {
    data: []u8,
    names: std.ArrayList([]u8),

    /// Caller owns the result; free it with `deinit`.
    fn init(gpa: Allocator, n: usize, first: []const u8) !Bag {
        const data = try gpa.alloc(u8, n);
        errdefer gpa.free(data);

        var names: std.ArrayList([]u8) = .empty;
        errdefer names.deinit(gpa);

        const name = try gpa.dupe(u8, first);
        errdefer gpa.free(name);

        try names.append(gpa, name);
        return .{ .data = data, .names = names };
    }

    fn deinit(self: *Bag, gpa: Allocator) void {
        for (self.names.items) |name| gpa.free(name); // elements first
        self.names.deinit(gpa);
        gpa.free(self.data);
    }
};

fn initAndDeinit(gpa: Allocator) !void {
    var bag: Bag = try .init(gpa, 16, "first");
    defer bag.deinit(gpa);
}

test "Bag has no leak on any allocation failure" {
    try std.testing.checkAllAllocationFailures(std.testing.allocator, initAndDeinit, .{});
}
```

Delete the `errdefer` on `data` or on `name` and that test fails with a leak report. Deleting the one on `names` still passes today, because no `try` follows the first allocation `names` owns; keep it anyway, since the next `try` added below `append` would leak the list. The test proves a pairing is needed only on the paths that exist, so the rule stays: pair every allocation.

## Leak tests

- Every test allocates through `std.testing.allocator` (a `SafeAllocator` in 0.17). A leaked byte fails the test.
- Every function with more than one allocation gets `std.testing.checkAllAllocationFailures(std.testing.allocator, f, .{extra args})`. It fails each allocation in turn, which is the only way the `errdefer` lines ever run. `f` takes the allocator as its first parameter and returns an error union.

What watches each build (from `lib/std/start.zig` in 0.17.0):

| Build | `init.gpa` | Leaks |
|---|---|---|
| `zig test` | `std.testing.allocator` | the test fails |
| Debug, ReleaseSafe | `SafeAllocator` over `page_allocator` | logged at exit; the exit code is unaffected |
| ReleaseFast, ReleaseSmall | `c_allocator` when linking libc, else `smp_allocator` | not checked |

Nothing watches ReleaseFast or ReleaseSmall, and Debug and ReleaseSafe only log, which is why the rungs put construction first. To make leaks fail a run, own the allocator: `var safe: std.heap.SafeAllocator = .init(std.heap.page_allocator, .{}); defer if (safe.deinit() != 0) @panic("leaked");`.

Done when every allocation has its pair on the next line, every owning type's `deinit` frees every owning field, and every multi-allocation function has a `checkAllAllocationFailures` test that passes.

## Sources

- TigerBeetle `docs/TIGER_STYLE.md`, "Safety" (static allocation) and "Cache Invalidation" (grouping allocation with deallocation)
- The `writing-leak-free-zig` skill (repo commit 2198aeb), ported from 0.16
- Zig 0.17.0 `lib/std/start.zig` (`callMain`) and `lib/std/heap/SafeAllocator.zig`
