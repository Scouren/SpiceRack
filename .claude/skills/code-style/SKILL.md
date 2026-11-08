---
name: code-style
description: House code style for Zig and Rust, as a checklist code must pass before it counts as done. Use when writing or reviewing a `.zig` or `.rs` file; when deciding whether to extract a helper or inline it; when ordering, splitting, or flattening files and folders; when deciding whether data belongs in a struct field, a parameter, or a local; or when asked whether code meets the house style.
---

# Code style

Code meets the style when any file reads top-down in one pass and every piece of data is alive exactly as long as something reads it. The checklist at the end is the bar; the sections above it are the rulings it checks against.

This skill owns the scale of one file and one function. Architecture (owners, spines, seams) is `codebase-clean-code`'s; data layout is `data-oriented-design`'s; the language idioms are the `zig` or `rust` skill's. Open the companion for the language you are writing before the first file: [zig.md](zig.md) or [rust.md](rust.md). Each carries a full file that passes every row below.

## Functions: extract or inline

A function body reads at one **level**: the steps of one job, each step a line of code or a call whose name says what the step does.

Extract a helper when one of these holds:

1. **Second instance.** The same logic is needed at a second call site. Compress it then (Muratori), named for what it means in the problem.
2. **Level break.** A run of detail (bit packing, a field parse, an index computation) sits between the caller's steps and has its own in → out: it reads at most four of the caller's values, returns one result, and touches no other local. Replaced by its call, the caller reads as a list of steps.
3. **Own tests.** It has a table of input → expected cases, or the test plan names it.

Keep the code inline when:

- Extracting it needs five or more parameters, an out-parameter into the caller's locals, or a context struct built only to make the call. The code is a step of the caller, so it stays a labelled block (`const header = blk: { ... };` in Zig, `let header = { ... };` in Rust) under a one-line `//` naming the step.
- Its body would be one call or one expression: call the inner thing directly.
- It is a step of a run-once sequence (a spine, `main`, an init). The sequence stays one function of labelled blocks, read top to bottom (Carmack).

Every helper is private and sits below the separator unless another module calls it.

## File order

Every `.zig` and `.rs` file follows one order:

1. **`//!` header**: in → out and what the code cites (`doc-comments`), then **Contents**, the signature of every `pub` item in the file. It comes first because both compilers reject `//!` after an import (Zig: `expected type expression, found 'a document comment'`; Rust: E0753).
2. **Imports and aliases**: std, then other modules, then siblings.
3. **Public types and constants.**
4. **Private types and constants.**
5. **Public functions**: the entry point or spine first, then the rest in the order it calls them.
6. **The separator**, exactly `// ---- private ----`.
7. **Private functions**, in the order they are first called above.
8. **Tests.**

So a function's helpers always sit below it, and a reader who only calls the file stops at the separator. Methods take the same split inside their type; the companion has the form for each language.

Contents is a copy of the declarations and goes stale silently; `check_style.py` diffs it against the file on every run.

## Files and folders

- **One transformation per file**, at most 500 code lines before the tests. A file past that splits along its transforms into sibling files of free functions, the split in `codebase-clean-code`'s HIERARCHY.md.
- **A folder holds two or more files.** A folder holding one file is a path segment that groups nothing: flatten it to `name.zig` / `name.rs` beside its siblings. Tool-mandated folders are exempt: a crate's `src/`, `tests/`, `benches/`, `examples/`, and `tests/**/common/mod.rs`.
- Name a file or folder for the data it owns (`physics.zig` owns `bodies`); the topic names (`utils`, `helpers`, `common`) are what HIERARCHY.md replaces.

## Where data lives

Every datum lives in the narrowest **scope** that outlives its last read. Each byte held past its last read raises the process's **high-water mark** (peak RSS) and buys nothing.

| The datum is | It lives in |
|---|---|
| read only inside one call | a local, out of scope before the call returns |
| read in several calls of one phase | a value the phase's caller owns (a spine local or the phase's arena), passed down as a parameter |
| read across the whole life of a struct | a field of that struct |
| never written after startup | a `const` table |

- A field read for the last time before its struct's `deinit` or drop is mis-scoped. The usual cases: a token buffer kept on the compiler after the AST is built, a file's bytes kept after parsing, a staging list kept after it was drained. Move it to a parameter or the phase arena, and free it at the phase's end, before the next phase allocates.
- A struct whose fields have different lifetimes is two structs: split it along the lifetimes, one struct per phase.
- Context reaches a function as a parameter; mutable globals are replaced the way LADDER.md's Singleton row says.
- A scratch buffer reused by every iteration of a hot loop may be a field, to amortize its allocation; its doc comment states its capacity. When the caller already has a scratch arena, pass that instead.

How Rust lifetimes and Zig slices express a scope is in the companion.

A claim that a change lowers memory carries the number: `command time -f '%M KB' <cmd>` (GNU time's maximum resident set size; `command` skips the shell's own `time`) before and after, and `valgrind --tool=massif <cmd>` then `ms_print` for which allocation sits at the peak.

## Public surface

Every `pub` item meets `api-design`'s rules, not only library boundaries:

- The call site is written before the signature.
- The caller owns memory: an allocator parameter named for its lifetime, or a caller-supplied buffer. Allocation and initialization are separate calls.
- Inputs are slices and plain values (`[]const T`, `&[T]`), never the callee's container types.
- A two-way choice is an enum (`.overwrite`, `.keep`) where a `bool` would read as a bare `true` at the call site.
- Units go last in names: `timeout_ms`, `len_bytes`.
- A struct with no invariant between its fields exposes them. A struct with one states it in its doc comment and asserts it where it is established; in Rust its fields are also private behind a constructor (Zig has no private fields).
- Errors are a named set or enum, each member's condition documented (`doc-comments`).
- An item is `pub` when another module calls it today.

## Pattern or function

The default is concrete code, then a plain function over plain data. A design pattern, trait, interface, generic, or function-pointer table enters only with three things written beside it: the variation axis, the second case that exists today (or the change drill that needs it), and why the rung below fails. The rungs, and the cheapest equivalent of each GoF pattern, are in `codebase-clean-code`'s LADDER.md: use the pattern's name to recognize the need, then build its equivalent.

## SIMD or the autovectorizer

| The loop | Write |
|---|---|
| cold, or under a few hundred elements | the scalar loop, and stop |
| hot Rust integer map, filter, or store | the plain loop; confirm vector instructions in the asm |
| hot Rust float reduction | explicit SIMD (`fearless_simd`): strict float order blocks the autovectorizer |
| hot Zig on 0.16 or 0.17 | explicit `@Vector`: the LLVM loop vectorizer is off |
| hot Zig on 0.18 | the plain loop, in the forms measured in `simd-loops`' zig.md |
| autovectorized but under the speed target, or must stay vectorized across edits and compiler upgrades | explicit SIMD |
| a data-dependent branch in the body | `branchless` first, then this table again |
| a chain dependency, variable-width elements, or data-dependent output length | a lane axis across independent instances (`simd-first.md` in the `zig` or `rust` skill); a single UTF-8 or JSON stream links simdutf or simdjson |

Every explicit SIMD loop keeps the scalar loop as its tail and its oracle, under a differential test. How to write each row is `simd-loops`'.

## Checklist

Run over every file the change touches. The mechanical rows come first: `python3 <this skill's dir>/check_style.py <path>...` checks header placement, Contents, file-level order, the separator, file size, and one-file folders, and prints `file:line: rule: message` per violation.

- [ ] **script**: `check_style.py` exits 0 over every touched file and the source root
- [ ] **level**: every function body reads at one level; every private function meets extract rule 1, 2, or 3; every run-once step that meets none is a labelled block
- [ ] **type order**: inside every type body, public members, then the separator, then private ones (the script checks file level only)
- [ ] **scope**: every struct field is read until its struct dies; no mutable global; each phase's data is freed before the next phase allocates; every memory claim has its before/after peak RSS
- [ ] **surface**: every `pub` item meets every Public surface line
- [ ] **rung**: every trait, interface, generic, vtable, or named pattern has its axis, its second case or drill, and its LADDER.md rung written beside it
- [ ] **lanes**: every hot loop has its SIMD row decided; every explicit SIMD loop has its scalar oracle and passing differential test
- [ ] **docs**: every header and `pub` item passes the `doc-comments` audit
- [ ] **modules** (Zig): every owner is a `build.zig` module imported by name, per [zig.md](zig.md)

Done when every box has a verdict, every failing box names its file:line, and the script exits 0.

## Keeping this skill true

The full files in zig.md and rust.md compile, pass their tests, and pass `check_style.py`. After editing the script, run `python3 check_style.py --self-test`; after editing either companion's file, compile it, test it, and run the script over it.
