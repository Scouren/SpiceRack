---
name: data-oriented-design
description: Data-oriented design — the data layout decides the code. Use when designing or reviewing struct layouts, SoA/AoS, entity or component tables, or field widths; when optimizing or profiling a hot path; or when the user mentions DOD, cache misses, ECS, or latency.
---

# Data-Oriented Design

> "The purpose of all programs is to transform data. If you don't understand the data, you don't understand the problem." — Mike Acton

## Six Questions (answer before writing ANY code)

State each answer explicitly before writing code — six stated answers is the bar; an unanswered question means stop and find out.

1. **What data goes in, what comes out?** State the transformation.
2. **How many?** Where there is one, there are many. Design for the collection, not the element.
3. **How wide?** The range every field can span, from the spec — not from the default integer type. `[0, 255]` is a `u8`. A range you cannot state is a spec gap: go ask, don't reach for `u64` to cover the ignorance. See Types for the widths.
4. **What's the access pattern?** Which fields are read together, how often, in what order — this decides the layout (Rule 2).
5. **What's the lifetime?** Phase (arena), program (static), or individual (pool). Match allocator to lifetime.
6. **Is it parallelizable?** Disjoint partitions of the same transformation → design for data parallelism from the start.

## Core Rules

1. **Know your data first.** Design the layout from the six answers above.

2. **SoA by default.** Store parallel arrays of each field, not an array of structs. Switch to AoS ONLY when you always access all fields of an entity together in the same loop.

3. **Hot/cold split.** Fields accessed every frame (position, velocity) go in one struct. Fields accessed rarely (debug name, creation timestamp) go in a separate cold-data table indexed by the same ID.

4. **Existence-based processing.** If an entity has a component, it exists in that system's array. Never store a component with an `active: bool` or `Option<T>` to skip — add/remove from the array instead. No null checks, no type tags. Iterate what exists.

5. **Tables > Trees > Graphs.** Flat arrays with integer indices first. B-trees if you need ordered lookup. Pointer-heavy graphs are last resort. Indices into arenas, never raw pointers between objects.

6. **Arena allocation.** Allocate in bulk, free in bulk. Group by lifetime. An arena of `Bullet` lives and dies with the level — no per-bullet malloc/free.

7. **Sequential access.** Linear iteration over contiguous memory beats random access every time. Sort by access pattern, not by "logical" grouping. Prefetch-friendly > "clean" hierarchy.

8. **No hidden control flow.** (Zig/Acton) No virtual dispatch, no operator overloading that hides allocations, no implicit copies. Every branch and allocation visible at the call site.

Before writing or reviewing any struct layout, check it against the anti-patterns in [REFERENCE.md](REFERENCE.md) — each has a BAD/GOOD pair (cold fields in hot lines, existence flags, pointer graphs, per-element dispatch, HashMap-over-array) plus the layout decision table.

Optimizing or profiling hot code → [measurement.md](measurement.md): profile-first workflow, latency costs, branch prediction, false sharing, the optimization decision tree, caller-owns-the-loop.

## Workflows

### Designing a New Data Type

- [ ] List every field the type needs
- [ ] Write the range of each field beside it, then pick the narrowest type that holds it (Question 3)
- [ ] Classify each field: hot (accessed every tick/request) vs cold (occasional)
- [ ] Group hot fields into a dense struct; cold fields into a separate table
- [ ] Apply Rule 2 to pick SoA vs AoS
- [ ] Use indices into arenas, not pointers or references — width set by the arena's capacity
- [ ] Add a generation counter to indices if you need to detect stale handles

### Refactoring OOP to DOD

- [ ] Identify the base class hierarchy — flatten it
- [ ] Replace inheritance with composition: separate data tables per "component"
- [ ] Replace virtual dispatch: separate arrays per type first; switch on a tag only when kinds are few and bodies small
- [ ] Replace `HashMap<EntityId, Component>` with a dense packed array + sparse-to-dense map
- [ ] Delete getters/setters — expose the data directly or use bulk accessors

## Types

- **Parse, don't validate**: validate once at the boundary, return a refined type; downstream takes only the refined type and never re-checks.
- Sum types over flag-bags: an `enum` over a struct of bools whose combinations are mostly illegal.
- Newtypes for IDs, units, indices — a `UserId` cannot be confused with a `PostId`.
- **Narrowest type the range allows** (Question 3). Width is derived, never defaulted:

  | Stated range | Type |
  |---|---|
  | `[0, 1]`, or a handful of named states | `bool`, or `enum(u8)` / packed bitfield |
  | `[0, 255]` | `u8` |
  | `[0, 65535]` | `u16` |
  | `[0, 4294967295]` | `u32` |
  | genuinely unbounded, or a real memory address | `usize` / `u64` |

  This is where SoA pays off twice: an `id: u32, hp: u16, team: u8` row costs 7 bytes packed in parallel arrays and 12 padded as a struct — nearly twice the entities per cache line for free. Index types follow the same rule: an arena capped at 65k entries is indexed by `u16`, not `usize`. Reserving the max value as a niche (`u16::MAX` = none, or `NonZero`) buys the `Option` back for zero bytes.

  Narrower is not free in two places, both checkable: an accumulator must be wide enough for the *sum*, not the element (summing `u8` into `u8` overflows), and a field whose range grows later is a breaking change — widen deliberately, don't pre-widen defensively.
- **Encode, don't polymorphize** — ranked, not interchangeable: existence-based table split (separate array per kind) first; closed `enum` + `match` on a tag only when kinds are few and bodies small; `dyn`/vtable only for genuinely open, cold sets. The full ladder with each rung's cost: the `codebase-clean-code` skill's `LADDER.md`.

## Review Checklist

Run against any code — each a checkable question, no vibes:

- [ ] **transformation** — can you state bytes-in/bytes-out for every function?
- [ ] **layout** — collections match access pattern; no pointer that could be an index; no bool/rare field in hot struct; every field's width justified by a stated range; no `dyn` in hot loop that could be an enum
- [ ] **memory** — caller owns allocation; nothing grows element-wise in a loop; nothing allocates per-iteration that an arena or reused buffer could hoist
- [ ] **parallel** — shared mutable state that could be partitioned; per-thread hot data on separate cache lines
- [ ] **types** — no flag-bag that should be a sum type; boundaries parse once (see Types)
- [ ] **compression** — no abstraction that wasn't discovered by repetition
- [ ] **verify** — hot loops confirmed to vectorize, or the blocking branch/access identified (a data-dependent branch is `/branchless`'s job, then `/simd-loops`)

## Language Companions

Auto-detect project language, load the companion:

| Language | Companion | Key primitives |
|----------|-----------|---------------|
| Rust | [rust.md](rust.md) | `soa_derive`, `slotmap`, `bumpalo`, `rayon`, `enum` + `match`, `NonZero*` |
| Zig | [zig.md](zig.md) | `MultiArrayList`, `enum(u32)`, `packed struct`, `ArenaAllocator`, `@Vector` |

The companions cover layout. Writing the language itself (idioms, memory discipline, the zero-cost probe) is the `zig` or `rust` skill.

See [foundations.md](foundations.md) for the theory (Acton, Kelley, Muratori — Parts I–III).
