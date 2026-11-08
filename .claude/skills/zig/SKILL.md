---
name: zig
description: Clean, zero-cost Zig 0.17 with no runtime residue. Use when writing or reviewing a `.zig` file; when porting Zig 0.16 code to 0.17; when deciding whether a Zig abstraction costs anything at run time; or when choosing the SIMD lane axis of a Zig program.
---

# Zig 0.17, clean and zero-cost

Target: **Zig 0.17.0**. The leading word is **residue**: the run-time work an abstraction leaves behind beyond the hand-written code. Clean Zig here means every abstraction on a hot path leaves zero or negative residue, and a codegen probe in `zig build test` proves it.

## Steps

1. **Pin the version.** `zig version` prints `0.17.0`. Code carrying 0.16 forms gets ported with [migration-0.17.md](migration-0.17.md). Done when that file's `rg` line prints nothing over the files you touch.
2. **Write with the idioms below.** Each construct in the change uses its row's form; open the linked file when the row's one line is not enough. Done when every row that applies to the change is applied.
3. **Memory.** Code that allocates follows [memory.md](memory.md). Done when every allocation has its `defer`/`errdefer` on the next line, every owning type's `deinit` frees every owning field, and every multi-allocation function has a passing `checkAllAllocationFailures` test.
4. **Residue.** Every new abstraction on a hot path (a generic, an interface, a handle type, a wrapper) gets a `gen_`/`hand_` pair in the probe, wired into `zig build test` ([zero-cost.md](zero-cost.md); start from [scripts/probe_template.zig](scripts/probe_template.zig)). Done when `scripts/asm_probe.py` reports FOLDED, IDENTICAL, EQUIVALENT or NEGATIVE for every pair, and every speed claim carries a benchmark number.
5. **Lanes.** A kernel over many independent problems is designed lane-first by [simd-first.md](simd-first.md): one kernel generic over `W`, `W == 1` as the oracle, a failure mask, a differential test. Per-loop vectorization goes to the `simd-loops` skill. Done when every vector kernel has a passing differential test against its `W == 1` instantiation.

Data layout (SoA, hot/cold splits, field widths) is decided first, by the `data-oriented-design` skill. Assert style is the `assertions` skill's; `pub` doc comments are the `doc-comments` skill's.

## Idioms

| Idiom | One line | Detail |
|---|---|---|
| comptime interface | `fn f(comptime F: type, x: F)` with `@hasDecl` checks; FOLDED. A ctx + fn pointer only where calls are per batch | [comptime.md](comptime.md) |
| `@hasDecl` | sees only `pub` decls (0.17): methods a duck-typed interface needs are `pub` | [comptime.md](comptime.md) |
| comptime existence | `field: if (has_x) T else void` takes 0 bytes; replaces `?T` on per-element hot paths | [comptime.md](comptime.md) |
| instantiation cost | each comptime value is a new body (~264 B here); runtime constants go through a parameter | [comptime.md](comptime.md) |
| `inline` prongs | `inline .a, .b => \|v, tag\|` instantiates one body per tag with `tag` comptime-known | [comptime.md](comptime.md) |
| decl literals | `pub const empty: Foo = ...` and `pub fn init(...) Foo`, used as `.empty`, `.init(...)` | [comptime.md](comptime.md) |
| reflection | `@typeInfo(T).@"struct".field_names` / `.field_types` (struct of arrays); `@FieldType(T, "x")` | [comptime.md](comptime.md) |
| fixed tables | `std.StaticStringMap(V).initComptime(...)` for string sets; `std.enums.EnumArray(E, V)` for enum-keyed arrays | [comptime.md](comptime.md) |
| handles | `enum(u32) { none = maxInt(u32), _ }`, converted with `@backingInt` / `@fromBackingInt`; FOLDED against raw `u32` | [zero-cost.md](zero-cost.md) |
| layout | `?u32` is 8 B, a sentinel enum 4 B, `?*T` 8 B; auto structs reorder fields | [zero-cost.md](zero-cost.md) |
| call conventions | read-only input by value (no copy, even at 128 B), mutation through `*T`, hot loops over primitive args | [zero-cost.md](zero-cost.md) |
| tagged unions | `union(enum)` + exhaustive `switch`; IDENTICAL to a hand tag-and-payload | [zero-cost.md](zero-cost.md) |
| packed fields | `packed struct(u8)` field reads; NEGATIVE against hand shifts | [zero-cost.md](zero-cost.md) |
| state machines | labeled `switch` with `continue :state next`; same dispatch as `while` + `switch`, explicit transitions | [zero-cost.md](zero-cost.md) |
| cold paths | `@branchHint(.cold)` as the first statement of the unlikely block | [zero-cost.md](zero-cost.md) |
| asserts | up-front length asserts remove per-iteration bounds checks in ReleaseSafe | [zero-cost.md](zero-cost.md) |
| multi-object loops | `for (0..n, xs, ys) \|i, x, *y\|`: one length check up front, lengths must be equal | [zero-cost.md](zero-cost.md) |
| allocator | always a parameter, named for its lifetime (`gpa`, `arena`, `scratch`) | [memory.md](memory.md) |
| arenas | one per lifetime: parse scratch, session, per query | [memory.md](memory.md) |
| `errdefer` | after every allocation that crosses a `try`; no error capture in 0.17 | [memory.md](memory.md) |
| leak tests | `std.testing.allocator` + `std.testing.checkAllAllocationFailures` | [memory.md](memory.md) |
| `SafeAllocator` | `std.heap.SafeAllocator.init(std.heap.page_allocator, .{})`; `deinit()` returns the leak count | [migration-0.17.md](migration-0.17.md) |
| formatting into memory | `gpa.print(fmt, args)` replaces `std.fmt.allocPrint` | [migration-0.17.md](migration-0.17.md) |
| list tail | `list.last()` / `list.last().?` / `list.lastPtr()` replace `getLast*` | [migration-0.17.md](migration-0.17.md) |
| repeated arrays | `const a: [n]T = @splat(v);` replaces `**` | [migration-0.17.md](migration-0.17.md) |
| rounding up | `@divCeil(n, w)` for chunk counts | [migration-0.17.md](migration-0.17.md) |
| lane masks | `const m: @Int(.unsigned, W) = @bitCast(cmp);` (`std.meta.Int` is gone) | [simd-first.md](simd-first.md) |
| vector code | 0.17: written as explicit `@Vector(W)` (the loop vectorizer is off). 0.18: per-element loops are plain loops in the measured forms, with gated fast-math only for float sums; lane-axis kernels stay explicit `@Vector(W)` | [simd-first.md](simd-first.md), simd-loops [zig.md](../simd-loops/zig.md) |

## Keeping this skill true

Every ```zig block in these files compiles and passes under `zig test` on 0.17.0, except the one headed `// build.zig`, which is verified in a real `zig build test`. After editing any file here, run `python3 scripts/check_snippets.py <path-to-zig-0.17>`. Done when it prints `check_snippets: N blocks compiled and passed`.
