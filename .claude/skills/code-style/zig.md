# Zig companion

Zig 0.16 and 0.17. Every block here was compiled and run on 0.16.0; the `build.zig` block in a real `zig build test`.

## A file that passes

Header, aliases, public types, the entry point, the separator, helpers in call order, tests. `classify` and `runEnd` are extract rule 2 (each a level break with its own in → out).

```zig
//! Tokenizer: source bytes → `Tokens` (one tag and one start offset per run).
//! Cites: none (house grammar, docs/grammar.md).
//!
//! Contents:
//! - `pub const Tag = enum(u8)`
//! - `pub const Tokens = struct`, `Tokens.deinit(tokens: *Tokens, gpa: Allocator) void`
//! - `pub const Error = error{ OutOfMemory, SourceTooLong }`
//! - `pub fn tokenize(gpa: Allocator, src: []const u8, out: *Tokens) Error!void`

const std = @import("std");
const Allocator = std.mem.Allocator;
const assert = std.debug.assert;

pub const Tag = enum(u8) { ident, number, space, punct };

/// `tags` and `starts` always have equal length; `tokenize` asserts it on return.
pub const Tokens = struct {
    tags: std.ArrayList(Tag) = .empty,
    starts: std.ArrayList(u32) = .empty,

    pub fn deinit(tokens: *Tokens, gpa: Allocator) void {
        tokens.tags.deinit(gpa);
        tokens.starts.deinit(gpa);
    }
};

pub const Error = error{ OutOfMemory, SourceTooLong };

/// Appends one token per run of same-tag bytes. On error, `out` holds the tokens before the failing run.
pub fn tokenize(gpa: Allocator, src: []const u8, out: *Tokens) Error!void {
    if (src.len > std.math.maxInt(u32)) return error.SourceTooLong;
    var i: u32 = 0;
    while (i < src.len) {
        const tag = classify(src[i]);
        const end = runEnd(src, i, tag);
        try out.tags.append(gpa, tag);
        try out.starts.append(gpa, i);
        i = end;
    }
    assert(out.tags.items.len == out.starts.items.len);
}

// ---- private ----

fn classify(c: u8) Tag {
    return switch (c) {
        'a'...'z', 'A'...'Z', '_' => .ident,
        '0'...'9' => .number,
        ' ', '\t', '\n' => .space,
        else => .punct,
    };
}

fn runEnd(src: []const u8, start: u32, tag: Tag) u32 {
    var i = start + 1;
    while (i < src.len and classify(src[i]) == tag) i += 1;
    return i;
}

test "tokenize splits runs by tag" {
    const gpa = std.testing.allocator;
    var tokens: Tokens = .{};
    defer tokens.deinit(gpa);
    try tokenize(gpa, "ab 12", &tokens);
    try std.testing.expectEqualSlices(Tag, &.{ .ident, .space, .number }, tokens.tags.items);
    try std.testing.expectEqualSlices(u32, &.{ 0, 2, 3 }, tokens.starts.items);
}
```

## Inside a type

A type body takes the file's order: fields, public declarations, `// ---- private ----`, private declarations. The compiler keeps the fields together (`declarations are not allowed between container fields`), so they always lead.

A file built around one type is the type: name it `Tokenizer.zig`, put the fields at top level after the imports, and the file order applies unchanged.

## Scope in Zig

There is no borrow checker, so the scope rule from SKILL.md is carried by names and doc comments:

- An allocator parameter is named for the lifetime it serves (`gpa`, `phase_arena`, `scratch`), per the `zig` skill's memory.md. A phase's data goes in that phase's arena, and the arena's `deinit` at the phase end is the free.
- A slice field that points into an arena makes its struct a view: its doc comment states that the struct must not outlive the arena (`doc-comments` rank 3, ownership).
- References stored in fields are index handles, never pointers (`codebase-clean-code`).

## Modules

Each owner (the module that writes a table, in `codebase-clean-code`'s sense) is its own `build.zig` module. The module graph is the unit the build schedules, so a fine-grained module DAG is what lets per-module parallel compilation pay off once Zig ships it. Until then it costs nothing and the compiler enforces the boundaries today.

- **Import other modules by name**: `@import("lex")`. A path import of a file that belongs to another module fails as soon as both are compiled: `error: file exists in modules 'lex' and 'root'`. Path imports (`@import("expr.zig")`) are for files of your own module.
- **A module with two or more files gets a directory** with `root.zig` at its top. The directory is then the module's path, and an import that climbs out of it fails to compile (`error: import of file outside module path`). A one-file module stays `src/name.zig`, so no folder holds a single file.
- **`root.zig` re-exports only what other modules call**, and the module's other files never import their own root (HIERARCHY.md).
- **List modules leaves first** in a `module_specs` array, and resolve each import only against the modules above it. A cycle cannot be written, because importing a module from below panics the build: `module 'lex' imports 'parse', which is not above it in module_specs`.
- **One test artifact per module**, so each module's tests compile alone against only its declared imports.

For `src/lex.zig` and `src/parse/{root,expr}.zig`:

```zig
// build.zig
const std = @import("std");

const Spec = struct { name: []const u8, path: []const u8, imports: []const []const u8 };

/// The module DAG, leaves first. A module may import only modules listed above it.
const module_specs = [_]Spec{
    .{ .name = "lex", .path = "src/lex.zig", .imports = &.{} },
    .{ .name = "parse", .path = "src/parse/root.zig", .imports = &.{"lex"} },
};

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const test_step = b.step("test", "Test every module");

    var made: std.StringArrayHashMapUnmanaged(*std.Build.Module) = .empty;
    for (module_specs) |spec| {
        const module = b.createModule(.{ .root_source_file = b.path(spec.path), .target = target, .optimize = optimize });
        for (spec.imports) |dep| {
            const dep_module = made.get(dep) orelse
                std.debug.panic("module '{s}' imports '{s}', which is not above it in module_specs", .{ spec.name, dep });
            module.addImport(dep, dep_module);
        }
        made.put(b.allocator, spec.name, module) catch @panic("OOM");
        test_step.dependOn(&b.addRunArtifact(b.addTest(.{ .root_module = module })).step);
    }
}
```
