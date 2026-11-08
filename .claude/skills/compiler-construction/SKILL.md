---
name: compiler-construction
description: Compiler frontends and passes built as stages over rows. Use when building or reviewing a lexer, parser, AST, IR, interner, semantic analysis, or lowering pass in Zig or Rust; when a pass recurses on input-controlled depth; or when designing diagnostics, conformance fixtures, or golden tests for a compiler.
---

# Compiler construction

A compiler is a chain of **stages**. Each stage reads the previous stage's **rows** and appends its own: tokens, nodes, IR instructions, diagnostics. A row is a fixed-width record in a column store, addressed by a `u32` index, and its tag says how to read the rest of it. Every rule below is about keeping rows small and stages narrow. Layout questions in general (field widths, SoA vs AoS, lifetimes) belong to `data-oriented-design`, and the spine and single-owner rules to `codebase-clean-code`. This skill covers what is specific to compilers.

## The rules

**1. Tokens and nodes are rows.** A token is `{tag: u8, start: u32}`, stored SoA, which is 5 bytes. Its end is re-lexed when someone needs it. A node is `{tag: u8, main_token: u32, data: {lhs: u32, rhs: u32}}`, which is 13 bytes SoA. A payload that doesn't fit two words goes into one shared `extra_data: []u32`, and `data` holds the offset. Variable-arity lists are stored as `[len, e0, e1, ...]` or as a `{start, end}` range. Big literals go in typed side tables. Each tag gets a doc comment saying how to read `lhs`, `rhs` and its extra words, and a comptime assert pins the width (`@sizeOf(Tag) == 1`, `@sizeOf(Data) == 8`). This is Zig's own `std.zig.Ast`. In Kelley's talk on it, tokens went from 64 B to 5 B and AST nodes from 120 B to 15.6 B on average. IR instructions went from 54.0 B to 20.3 B. Wall clock fell 22% in tokenize+parse and 39% in IR generation. Those are the talk's numbers, not re-measured here.

**2. Intern once, compare by index.** Every identifier and string goes into an intern pool when it is lexed or parsed. After that, every stage compares `StrId == StrId` and indexes per-name tables by the id. The pool is a byte buffer plus offsets, with a hash map whose key is the 4-byte id and whose hash reads the stored bytes through an adapter. VerA measured 5 B a slot this way against 21 B for keying by slice. Zig's `InternPool` goes further: types and values are interned too, so two values of the same type are equal exactly when their indices are equal. Ids are given out in insertion order, so the same input gives the same ids. Determinism (rule 9) depends on that.

**3. Stage outputs are narrow values.** Each stage returns one named type, and the next stage takes it as `*const` / `&`. It can't reach the producer's symbol tables, scratch state or allocator. VerA's lowering returns `Lowered`, and nothing downstream sees the lowering internals. In rust-analyzer the syntax tree is a value type that knows nothing about semantics, and the parser emits a flat event stream that is independent of the tree. A stage that needs something from two stages back has found a missing column in the stage between.

**4. Per-opcode facts are table columns.** Everything known about a tag or opcode lives in one row per variant: operand class, result type, arity, whether it is foldable, a predicate flag, the spec domain. Build it so a missing row fails to compile. In Zig that is `std.EnumArray(Tag, Info).init(.{...})`, which errors with `missing struct field: <tag>`. In Rust it is a `const fn info(t: Tag) -> Info` whose `match` has no `_` arm. Target spellings get a separate table owned by the backend. Every `switch` over a stage-boundary enum names every prong. Where an `else =>` is truly right, say why on that line. VerA enforces this with a test that parses its own source using `std.zig.Ast` and fails on any `else =>` over a boundary enum that has no `// else: <reason>`. Use exhaustive prongs wherever a string compare or a silent default would otherwise go.

**5. Diagnostics are data with codes.** A stage appends `Diag{code, span/token, ...}` rows to a list and keeps going. Its result is `(T, errors)`, the rust-analyzer invariant, and Zig's `Ast.errors: []const Error` has the same shape. Rendering happens once, at the end, after sorting into source order. A `Code` enum is the single catalogue:

- each code gets a title, the spec clause, and `--explain` text
- the first letter is the severity
- the leading digits are the owning stage class
- codes are never renumbered or reused; a retired code keeps its slot

Tests, fixtures and user scripts pin the code. They never pin the message text.

**6. Parse in full, then refuse by clause.** The parser accepts the whole grammar from day one. When a construct parses but no later stage can execute it yet, refuse it at that stage with a named diagnostic that cites the clause. "Compiled, and the block silently did nothing" is a wrong answer that carries no diagnostic. A refusal is a correct answer. Keep a gate shut until an executor stands behind it.

**7. Walks over input-controlled depth use worklists.** Expression trees, statement lists, def-use chains, dominator trees and dependency chains all nest as deep as the input. Walk them with an explicit stack of `{node, state}` frames on the heap.

- Measured here: a recursive evaluator over `1 + (1 + ...)` segfaults between 10⁴ and 10⁵ levels in Debug on a 16 MiB stack. The worklist version runs 10⁶ levels in Debug.
- VerA's recursion overflowed at 2×10⁴ chained statements.
- Where emitted bytes depend on visit order, the worklist reproduces the recursive order exactly: push children reversed, re-test at pop time (VerA `ir/ssa.zig`'s trivial-phi worklist).
- A recursive-descent parser either caps nesting with a named diagnostic or runs on a reserved stack sized from a measured bytes-per-level (VerA `frontend/deep_stack.zig`).
- Done when a fixture nested 10⁴ levels deep (more if the spec allows it) passes in Debug.

**8. The compiler stays scalar; the code it emits is where SIMD goes.** A compiler walks each instruction once, along chains (dominator trees, output that depends on the data, SSA construction). VerA measured this: its lattice-fill table (`lib/ir/proof/lattice.zig`) shows an SoA `@Vector` fill losing to plain `@memset` at 24,578 values. At the corpus median of 26 MIR values, no SoA split paid off at all, so the AoS layout shipped. A re-triage on 2026-09-23 found nothing else that pays. The only exceptions it keeps are byte scans in the lexer and preprocessor (a single-needle scan, a keyword-table accumulator), which belong to `simd-loops`. Spend SIMD effort on the emitted program's hot loop.

**9. Refactors keep goldens byte-identical.** Before a structural change, snapshot every emitted artifact (code, diagnostics rendering, dumps) across the whole fixture corpus. Snapshot again after, and diff the bytes. The diff tool prints `IDENTICAL` or exits 1 and names each file that differs. Each refactor phase is one branch and one PR, and behaviour changes and bug fixes land as separate commits before or after it. If a phase can't stay byte-identical, re-scope it. Determinism is part of the same gate: compile twice and byte-compare.

**10. Conformance fixtures cite the spec.** Each fixture's header gives:

- the clause number and the sentence it tests
- an expected value derived by hand, never copied from the compiler's output
- the exact diagnostic code it expects
- the number of checks it must report

A clause has two-way evidence when it has a positive fixture plus a refusal fixture with a legal neighbour: the invalid construct isolated, its code pinned, and a minimal legal variant shown to compile. An `xfail` marker fails on XPASS so it can't outlive its limitation. CI gates on the list of FAIL names, never on the count. Before you change the compiler to make a fixture pass, establish that the fixture is right. The header grammar and harness rules are in [FIXTURES.md](FIXTURES.md).

## Designing a new frontend or pass

1. **List the stages.** One line per stage: the rows it reads, the narrow type it returns, and the diagnostic class it owns. Done when every stage's output type has a name and no stage reads a type from two stages upstream.
2. **Lay out the rows.** Give each stage a tag enum, its `data` words and `extra_data` encodings, and side tables for wide literals. Run `data-oriented-design`'s six questions on each table. Done when every tag's doc comment says how to read its words and comptime asserts pin `@sizeOf(Tag)` and `@sizeOf(Data)`.
3. **Intern.** Set up one pool per id space and say which stage owns it. Done when no stage after the lexer or parser compares strings (grep `mem.eql(u8` / `==` on `&str` in the passes).
4. **Write the fact tables** (rule 4). Done when adding a throwaway variant fails to compile at every decision site, and you have counted those sites and reverted.
5. **Write the diagnostic catalogue** (rule 5): the code enum, stage-class digits, title, clause and explain text. Done when every error path in the stage appends a coded row, and every refusal (rule 6) names its clause.
6. **Flatten the walks** (rule 7). List every walk whose depth the input controls. Done when each one is a worklist or has a named depth cap, and the deep fixture passes in Debug.
7. **Write fixtures per clause** (rule 10), with [FIXTURES.md](FIXTURES.md) open. Done when the clause→fixture map shows two-way evidence for every clause the stage implements, or a written reason where a rule has no invalid form.
8. **Wire the goldens** (rule 9). Done when `golden before` / `golden after` / `golden diff` run on the whole corpus and a double-compile byte-compare is in CI.

Each new file opens with a `//!` header giving its transformation (in → out) and the clauses it cites. The rest of the comment rules are in `doc-comments`. Test layers and oracles beyond fixtures and goldens (differential testing, generators, fuzzing the parser) come from `test-plan`'s compiler section.

## Reviewing an existing frontend or pass

Reconstruct the stage list first, because most violations only show up against it. Then check:

- [ ] **rows**: tokens and nodes are SoA with `u32` indices; there are no pointers or slices in node rows; the tag is `u8`; every width is pinned by assert
- [ ] **extra_data**: each tag's doc says how to decode it; lists are `[len, ...]` or ranges, never a nested `ArrayList` per node
- [ ] **interning**: no string compare after parsing; ids are in insertion order
- [ ] **narrow outputs**: each stage takes its input as `*const` / `&`; list every reach-through into an upstream stage's internals
- [ ] **fact tables**: a missing row fails to compile; every `else =>` / `_ =>` over a boundary enum carries its reason
- [ ] **diagnostics**: every error is a coded row; no message-only error and no early `return error.X` that drops the rest of the input
- [ ] **refusals**: every construct that parses but doesn't execute is refused with a clause-citing code; no silent no-op path
- [ ] **depth**: every input-controlled walk is a worklist or capped with a code; the deep fixture is present
- [ ] **scalar**: no `@Vector` / `std::simd` in a pass, except a measured byte scan in the lexer or preprocessor
- [ ] **goldens**: a byte-identical diff tool exists; a determinism check is in CI
- [ ] **fixtures**: two-way evidence per clause; refusals have legal neighbours; CI gates on FAIL names

Done when every box has a verdict with a `file:line`, and each failure names the rule it breaks.

Compiled layouts of everything above, ready to copy, are in [zig.md](zig.md) for Zig 0.17 and [rust.md](rust.md) for Rust 1.95 (edition 2024).

## Sources

- Andrew Kelley, *A Practical Guide to Applying Data-Oriented Design*, Handmade Seattle 2021: media.handmade-seattle.com/practical-data-oriented-design (numbers from the transcript at josherich.me/podcast/andrew-kelley-practical-data-oriented-design-dod)
- Zig 0.17 `lib/std/zig/Ast.zig` (`TokenList`, `Node.Data`, `extra_data`, `extraData`, `errors`)
- Zig compiler `src/InternPool.zig` (`Item{tag, data}`, index equality): github.com/ziglang/zig
- rust-analyzer architecture: github.com/rust-lang/rust-analyzer/blob/master/docs/book/src/contributing/architecture.md (parser invariants, `(T, Vec<Error>)`, data-driven tests)
- VerA `AGENTS.md` §2, §4, §5, §6; `lib/frontend/ast.zig`, `lib/ir/opcode.zig`, `lib/diag_code.zig`, `lib/diag/entry.zig`, `lib/ir/ssa.zig`, `lib/ir/lower/contrib.zig`, `lib/frontend/deep_stack.zig`, `lib/ir/proof/lattice.zig`, `tests/exhaustive.zig`
- ESPice `src/frontend/netlist.zig` (three walks over logical lines, values as flat rows), `src/frontend/csr.zig` (CSR hypergraph, SoA payload), `src/core/intern.zig`
