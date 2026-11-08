# Doc comments in Zig

Zig has no borrow checker and no `missing_docs`. The compiler will not tell you a contract is undocumented; it only tells you a comment is in the wrong place. Everything that matters is convention enforced by review, with one exception: the doctest, a `test` named after the declaration, which `zig test` runs.

Every behaviour below was checked with Zig 0.17.0.

## Placement is a compile error

| Mistake | 0.17 error |
|---|---|
| `//!` after any declaration, e.g. after `const std = @import("std");` | `expected type expression, found 'a document comment'` |
| `///` with nothing after it | `unattached documentation comment` |
| `///` on a `test` block | `documentation comments cannot be attached to tests` |
| `///` on a local inside a function body | `expected statement, found 'a document comment'` |

So the `//!` header is the first thing in the file, and inside a body only `//` is legal. `zig fmt --check` passes on a file that gets these right.

## The file header

The VerA rule: every file opens with a `//!` header stating its transformation (in → out) and the clauses its code cites. Real headers to copy:

```zig
//! If-conversion: MIR → the same MIR with each pure CFG diamond or triangle
//! replaced by straight-line code and one §4.2.12 `select` per join phi, to a
//! fixpoint. An arm converts only when every instruction is a pure value op
//! (no call, `opt_barrier`, store or domain-restricted op), so §4.2.3's "side
//! effects shall not occur" holds by construction.
```

```zig
//! The bag: every diagnostic of one compilation, collected, deduplicated and
//! sorted. In: `Builder` commits from every stage. Out: an ordered table of
//! `Record` rows whose text lives in one string pool.
```

```zig
//! The C ABI over `Problem`, built as libespice.a. include/espice.h is the
//! contract: every export's behaviour and every struct layout is documented
//! there, and the comptime block below fails the build if the enums drift.
```

The first two are VerA (`lib/ir/ifconv.zig`, `lib/diag/bag.zig`), the third ESPice (`src/c_api.zig`). When a file cites many clauses, list them on their own line so the audit can diff the list against the body: `//! LRM clauses this file's code cites: §1.3.1, §2.7, ...` (VerA `lib/ir/lower/tables.zig`). When it cites none, say so and why. VerA's `lib/ir/verify.zig` goes further and lists the invariants it deliberately does not check, each with its reason; that is the header's rank-6 line.

## The three stdlib phrasings

Ranks 1–3 of the ladder are the only place this information exists in Zig. The stdlib phrases them near-verbatim across many declarations; match the wording so a grep finds them. Counts are from the 0.17 `lib/std`:

| Rank | Line | Uses |
|---|---|---|
| Precondition | `Asserts that the index is in bounds or equal to the length.` | 14, plus 131 `/// Asserts that ...` lines overall |
| Invalidation | `Invalidates element pointers if additional memory is needed.` | 33 |
| | ``Invalidates pre-existing pointers to elements at and after `index`.`` | 14 |
| Ownership | `Caller owns returned memory.` / `The caller owns the returned memory.` | 8 each |
| | ``Caller owns returned string and must free with `allocator`.`` | 2 |

`Asserts` is Zig's `# Panics`; `Invalidates` is Zig's borrow checker. A `*Self` method that may reallocate without an `Invalidates` line is a use-after-free waiting for a caller. Name the allocator in the ownership line whenever the function takes more than one.

`std.ArrayList.insert` in 0.17 is the order to copy, *what → equivalence → cost → invalidation → asserts*:

```zig
/// Insert `item` at index `index`. Moves `list[index .. list.len]` to higher indices to make room.
/// If `index` is equal to the length of the list this operation is equivalent to append.
/// This operation is O(N).
/// Invalidates element pointers if additional memory is needed.
/// Invalidates pre-existing pointers to elements at and after `index`.
/// Asserts that the index is in bounds or equal to the length.
pub fn insert(self: *Self, gpa: Allocator, index: usize, item: T) Allocator.Error!void {
```

Five of the six lines state something the signature cannot.

## The error set

An inferred `!T` tells the caller nothing about when each error happens. Give a public function a named error set and document it in two places:

- **On the set**: what holds after any error. Is the output untouched, partially written, or rolled back? Is a partial result returned?
- **On each member**, with its own `///`: the condition that produces it, in the caller's terms.

```zig
/// Every way `generate` fails. Out of memory aside, each is a refusal of
/// the whole device: no partial `Output` is returned. A refusal confined to
/// one unit is not an error here but a `@compileError` body plus `fatal_out`.
pub const Error = std.mem.Allocator.Error || error{
    /// proof.zig rejected the model. `generate` returns this before emitting
    /// a byte.
    DomainErrors,
    /// A numeric parameter default cannot follow host-written dependencies.
    UnsupportedParameterDefault,
};
```

That is VerA's `lib/backend/codegen.zig`; the 0.17 stdlib documents members the same way (`std.http` `BodyError`, `std.Io` `UnexpectedError`). An error set with one or two members may carry the conditions in the set's own comment instead, one line per member (VerA `lib/diag/level.zig` `SetError`).

## A complete file

This compiles, passes `zig fmt --check`, passes `zig test`, and builds docs with `zig build-lib -femit-docs` under 0.17:

```zig
//! Line splitter: a netlist's bytes in, one `Line` row per logical line out,
//! with `+` continuations joined (§2.1) and `*` comment lines dropped (§2.3).
//! Rows index into the input; nothing is copied.

const std = @import("std");
const assert = std.debug.assert;
const Allocator = std.mem.Allocator;

/// Most lines one netlist may hold. A deck past this is refused, not truncated.
pub const lines_max: u32 = 1 << 20;

/// One logical line: a byte range of the input.
pub const Line = struct { start: u32, end: u32 };

/// Every way `split` fails. On any error `out.items` is left as it was.
pub const Error = Allocator.Error || error{
    /// A `+` continuation came before any line it could continue.
    OrphanContinuation,
    /// The input holds more than `lines_max` logical lines.
    TooManyLines,
};

/// Appends one row per logical line of `text` to `out` and returns how many.
/// Rows are byte offsets into `text`; they mean nothing for any other buffer.
/// Asserts that `text` is shorter than 4 GiB.
/// Invalidates element pointers of `out` if additional memory is needed.
pub fn split(gpa: Allocator, text: []const u8, out: *std.ArrayList(Line)) Error!u32 {
    assert(text.len < std.math.maxInt(u32));
    const len_before = out.items.len;
    errdefer out.shrinkRetainingCapacity(len_before);
    var it = std.mem.splitScalar(u8, text, '\n');
    var count: u32 = 0;
    while (it.next()) |raw| {
        const start: u32 = @intCast(raw.ptr - text.ptr);
        const end: u32 = start + @as(u32, @intCast(raw.len));
        if (raw.len == 0 or raw[0] == '*') continue;
        if (raw[0] == '+') {
            if (count == 0) return error.OrphanContinuation;
            out.items[out.items.len - 1].end = end;
            continue;
        }
        if (count == lines_max) return error.TooManyLines;
        try out.append(gpa, .{ .start = start, .end = end });
        count += 1;
    }
    assert(out.items.len == len_before + count);
    return count;
}

test split {
    const gpa = std.testing.allocator;
    var lines: std.ArrayList(Line) = .empty;
    defer lines.deinit(gpa);
    const text = "R1 a b\n+ 1k\n* comment\nC1 b 0 1p";
    try std.testing.expectEqual(2, try split(gpa, text, &lines));
    try std.testing.expectEqualStrings("R1 a b\n+ 1k", text[lines.items[0].start..lines.items[0].end]);
    try std.testing.expectError(error.OrphanContinuation, split(gpa, "+ 1k", &lines));
    try std.testing.expectEqual(2, lines.items.len);
}
```

The last line of the test executes the set-level promise ("on any error `out.items` is left as it was"). The precondition is an assert in the body and the failure lines are expectations in the test; only the invalidation line rests on prose alone.

## The example is a named test

`test <declName>` binds the test to the declaration, and autodoc renders it under that declaration's "Example Usage" heading (`lib/docs/index.html` in 0.17). A string-named test is right for a scenario spanning several declarations; the example that documents one public function uses the identifier form. The 0.17 stdlib carries 541 identifier-named tests (and 1,379 string-named ones).

```zig
/// Allocates a lower case copy of `ascii_string`.
/// Caller owns returned string and must free with `allocator`.
pub fn allocLowerString(allocator: std.mem.Allocator, ascii_string: []const u8) ![]u8 {

test allocLowerString {
    const result = try allocLowerString(std.testing.allocator, "aBcDeFgHiJkLmNOPqrst0234+💩!");
    defer std.testing.allocator.free(result);
```

The `defer ...free` does documentation work: `std.testing.allocator` fails the test on a leak, so the ownership line is executed on every `zig test`. Prefer demonstrating a contract in a doctest over stating it only in prose. TIGER_STYLE makes the same trade one level down: "you may use a blatantly true assertion instead of a comment as stronger documentation where the assertion condition is critical and surprising."

Write examples as named tests, not as fenced code inside `///`. Nothing compiles a fenced block in a Zig doc comment.

## Enforcement

```sh
zig fmt --check .                      # formatting; placement errors also fail here
zig build test                         # runs the doctests, the only mechanical check on doc content
zig build-lib -femit-docs=docs -fno-emit-bin root.zig   # autodoc: index.html, main.js, main.wasm, sources.tar
```

There is no `missing_docs` equivalent, so presence of docs is a review responsibility. That is why the audit pass in the skill is run deliberately rather than left to CI.

## Prose style

TIGER_STYLE: comments are sentences, with a space after the slashes, a capital letter, and a full stop. "Well-written prose describing the code, not just scribblings in the margin."

Sources: Zig 0.17.0 `lib/std` (`array_list.zig`, `ascii.zig`, `debug.zig`, `http.zig`, `Io.zig`); VerA `AGENTS.md` §4, `lib/ir/ifconv.zig`, `lib/ir/verify.zig`, `lib/ir/lower/tables.zig`, `lib/diag/bag.zig`, `lib/diag/level.zig`, `lib/backend/codegen.zig`; ESPice `src/c_api.zig`; TigerBeetle `docs/TIGER_STYLE.md`.
