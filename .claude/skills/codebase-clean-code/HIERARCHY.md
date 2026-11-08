# Hierarchy: system down to function

Each level exists for one reason, and a unit is created at a level only when that reason holds. Everything lower is grouped by **owner and dependency direction**: a unit sits where its writes and its imports put it, never where its topic would.

| Level | Exists to | Created when | Enforced by |
|---|---|---|---|
| System (process, binary) | isolate a failure domain, a trust domain, or a size budget | a separate binary or sandbox is required (an untrusted plugin, a headless core with its own size limit) | the OS or the runtime (wasm) |
| Package / crate | make ownership, a heavy dependency, or a platform split un-crossable | one of the three reasons below holds | the package manager and build graph |
| Module | own one set of tables | a group of transforms writes the same tables | build wiring, or a test where the build can't |
| File | hold one transformation | a transform has its own in → out | the `//!` header and the import rules below |
| Function | hold one cohesive step | the step is reused, documented, or tested on its own | review against the rules below |
| Block | sequence the steps of a function that runs once | always, inside a spine | — |

## Package and crate boundaries

A package or crate boundary exists only to:

1. **Enforce single ownership.** Other crates get `&T`, the owner keeps `pub(crate)` mutation, so a stray write does not compile (Schemify hard rule 3).
2. **Gate a heavy dependency.** `vello`/`wgpu` live in `render` only, `wasmi` in `plugin` only; the headless core links none of them (Schemify `crates/cli/tests/deps.rs` checks the resolved graph for every target).
3. **Split by platform.** Native vs web, CPU vs GPU kernels, behind `cfg` or a build option.

Anything else is a module inside an existing crate. A crate per topic multiplies compile units and public interfaces without enforcing anything.

## Group by owner, not by topic

A topic folder (`utils/`, `common/`, `models/`, `managers/`, `helpers.rs`) collects code that touches several owners' data. Each owner imports a helper from it, the helpers import the owners' types, and the cycle closes through the folder. The fix is placement:

- A helper that touches one owner's tables goes into that owner.
- A type shared by several owners goes into a **leaf** with no internal dependencies (Schemify `format`, `expr`, `abi`; ESPice `core`).
- A file goes in the lowest module that can import everything it needs. Where a file goes is decided by what it must import, not by what it is about (VerA §4: a planned `support/contract.zig` could not exist because its assertion needed three modules a bottom-of-stack file cannot import).

Name a module or file for the data it owns (`physics.zig` owns `bodies` and `contacts`), never for a pattern. Names carry units last, in descending significance (`latency_ms_max`).

## The build enforces the DAG

Review does not keep a dependency graph acyclic; the build does. What each toolchain checks, as measured (Rust 1.95, Zig 0.17):

| Toolchain | Checks | Leaves open | Close the gap with |
|---|---|---|---|
| Cargo | rejects a crate cycle (`error: cyclic package dependency`) | modules inside one crate may import each other in a cycle (compiles) | crates for owners whose direction matters; a test over `cargo tree` for forbidden edges |
| Zig `build.zig` | a module sees only the imports `build.zig` hands it (`error: no module named 'c' available within module 'b'`) | two modules handed to each other build fine: the build accepts a module cycle; files inside a module import each other freely | list modules in dependency order and resolve each import only against modules above it |

The Zig pattern, from VerA's `build.zig`: a `module_specs` array in dependency order, where `defineModules` looks each import up among the modules already created and panics otherwise. The array is the module graph, and a cycle cannot be written in it. ESPice gets the same effect by wiring each module's imports by hand (only `analysis` receives `solver`, so an import of `solver` anywhere else does not compile).

Inside a module, where the toolchain checks nothing:

- **Leaves never import their module's root.** The root (`root.zig`, `lib.rs`, `mod.rs`) aggregates and re-exports; leaves import shared types from a sibling `types.zig` or from other leaves. A leaf that imports its root closes a cycle through the aggregation (ESPice).
- **The root aliases only what other modules call.** That alias list is the module's API (VerA).
- **Stage outputs are narrow values.** A stage returns one value type (`Lowered`), and everything downstream takes `*const Lowered`, so it cannot reach the stage's scratch tables (VerA).

## Files: one transformation, header first

Each file holds one transformation and opens with a header that says what goes in and what comes out:

```zig
//! Lowering: checked AST + symbol tables → `Lowered` (MIR functions, constant pool).
//! Reads `ast`, `sema.Symbols`. Writes nothing outside the returned value.
//! Cites LRM §4.2, §9.1.
```

```rust
//! Connectivity: doc wires, labels and ports → `nets`, `pin_nets`.
//! Owner: elab. Reads doc tables; writes only the returned `CellNets`.
```

A source file reads top-down: types, then the spine (if it has one), then transforms in the order the spine calls them. A big file splits into sub-files of free functions that keep `self: *T` and are called directly (`lower_expr.lowerExpr(self, e)`, the pattern of Zig's own `src/Sema/*.zig`).

## Functions: cohesion, not line count

Function size follows cohesion. A spine or a run-once sequence stays one function with labelled blocks rather than a chain of one-call helpers (Carmack). Extract a function only when it is used in three or more places, defines an interface worth documenting, is named by the test plan, or has a table of input → expected-output cases. A 200-line function that does one thing in order is cohesive; a 20-line function that branches between three unrelated jobs is not.

- **Branches that choose which work to do live in the spine**, never inside a transform. A transform is a straight loop; a data-dependent select inside it is `branchless`'s domain.
- **Signatures are the data-flow graph.** Every table a function reads is a `const` parameter, every table it writes a mutable one. No globals, no stored back-pointers, no context struct carrying more than the function touches.
- **References are handles.** Cross-table references are index handles (with a generation counter where a handle can go stale). Pointers live inside a block, never in a field.
- **Limits and asserts.** Every table has a stated capacity; every transform asserts its entry invariants (parallel columns have equal length, handles are live) and one exit invariant (TigerStyle: two per function on average).
- **Boundary enums are matched exhaustively.** No `else =>` (Zig) or `_ =>` (Rust) over an enum that crosses a module boundary, so adding a variant makes the compiler list every site. VerA enforces it with a test that parses the tree and fails on `else =>` over a boundary enum unless the line carries `// else: <why this is right for every present and future variant>`.

## Sources

- Schemify `docs/dev/architecture.md` § Crates, `docs/dev/contributing.md` hard rules 2, 3, 7; `crates/cli/tests/deps.rs`
- ESPice `AGENTS.md` § The dependency DAG
- VerA `AGENTS.md` §4 and `build.zig` (`module_specs`, `defineModules`)
- John Carmack on inlined code: number-none.com/blow/john_carmack_on_inlined_code.html
- TigerBeetle, *TigerStyle*: github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md
- Andre Weissflog, *Handles are the better pointers*: floooh.github.io/2018/06/17/handles-vs-pointers.html
- Toolchain behaviour measured in the lab: a two-crate Cargo cycle, a two-module `build.zig` cycle, an unwired `@import`, and in-crate / in-module file cycles
