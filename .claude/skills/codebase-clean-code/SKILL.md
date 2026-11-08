---
name: codebase-clean-code
description: Architecture and abstraction decisions for systems code in Zig, Rust, or C, made by owner and dependency direction and priced on a cost ladder checked by change drills. Use when laying out a program or subsystem, deciding which crate, module, or file a feature belongs in, choosing between a pattern or abstraction and concrete code, translating a GoF pattern, or judging how cheap a codebase is to change.
---

# Architecture, cheapest rung first

An architecture is right when the changes the project will actually make are cheap and nothing else is paid for. Data comes first, then who owns it, then the order it flows in; abstractions come last and only where something really varies. The words every step below uses:

- **Tables** hold the data: collections named as plural nouns (`bodies`, `pending_writes`), laid out by `data-oriented-design`.
- **Transforms** hold the behaviour: functions over whole tables, named as verb phrases (`integrateBodies`). One transform per file.
- **The spine** is one function per pipeline (a frame, a request, a compiler pass) that calls every transform in order, one line each.
- **Owner**: the one module that writes a table. Everyone else reads it or sends a **request table** the owner drains.
- **Seam** (Feathers, as in `codebase-design`): a place where behaviour varies without editing that place. Every seam sits on one **rung** of the ladder in [LADDER.md](LADDER.md).
- **Change drill**: a named likely change and the full list of edits it needs. "Long-term" means "passes the drills"; without a drill it is speculation.

**Cost** has three axes, and every rung is priced on all three:

| Axis | What it counts | How you get the number |
|---|---|---|
| Runtime residue | work the seam leaves in the hot path next to concrete code: indirect calls, lost inlining, lost vectorization | asm probe, per the `zig` or `rust` skill's `zero-cost.md` |
| Change radius | existing function bodies a drilled change edits | the drill's edit list |
| Interface weight | concepts a caller must learn: types, ordering rules, error modes, registration | count them off the interface (`codebase-design` calls the inverse *depth*) |

Library and FFI boundaries are `api-design`'s; deepening one module's interface and the seam vocabulary are `codebase-design`'s; struct layout is `data-oriented-design`'s.

## Designing a program, subsystem, or feature

The order is **Data → Owners → Spines → Seams → Ladder → Drill → Record → Prove**. A feature in an existing program runs the same steps over only the tables it touches.

### 1. Data

Write down what enters and what leaves, then every table in between, then each transform as `name: reads → writes`. Start from the data and its transformations, not from the nouns of the domain: a noun list becomes a class list, and a class list becomes a topic folder. For each table run the six questions in `data-oriented-design`.

Done when every table has a producer and a consumer, a layout, a capacity, and a lifetime (an arena named for its phase: `frame_arena`, `level_arena`, `permanent`), and every transform is written as `reads → writes`.

### 2. Owners

Give each table exactly one owner: group transforms by the table they **write**, and those groups are the modules. Then order the modules by what they read: a module depends only on the owners of the tables it reads, and the arrows form a DAG. Grouping by topic (`models`, `utils`, `managers`, `types`, `common`) is what produces cycles, because a topic folder collects files whose imports point in both directions. When code outside the owner must change a table, it appends to a request table the owner consumes (`damage_events` → `applyDamage`).

Whether an owner becomes a crate, a module, or a file, and how the build enforces the DAG, is in [HIERARCHY.md](HIERARCHY.md); open it whenever you create, move, or split a crate, package, module, or file.

Done when the ownership map lists one owner per table, the module dependency graph has no cycle, and every write from outside an owner goes through a named request table.

### 3. Spines

Write one function per pipeline, one line per transform, before any transform body exists (Muratori's usage code first, applied to the program). Every branch that chooses *which* work happens (command dispatch, gating, kind selection) lives in the spine; transforms are straight loops (push ifs up, fors down). A transform's signature takes every table it reads as `const` and every table it writes as mutable, so the spine's signature list is the data-flow graph.

Done when the spine runs end to end as stubs, no transform calls another transform, and no transform body branches on which work to do (data-dependent selects inside a loop are `branchless`'s).

### 4. Seams

Find the variation axis before looking for a pattern. For each place where behaviour could differ, name what varies (kind, backend, platform, dialect, policy, plugin) and the evidence: **two cases exist today**, or a change drill from step 6 needs it. Muratori's rule applies: make it usable before making it reusable, and compress at the second real instance. One case and no drill means no seam; the code stays concrete.

Done when every seam has an axis and evidence written next to it, and every place without evidence is plain concrete code.

### 5. Ladder

Put each seam on the cheapest rung that handles its axis:

1 concrete code · 2 free function over plain data · 3 table + transform · 4 closed enum + exhaustive switch · 5 comptime or generic parameter · 6 request table or command queue · 7 function-pointer table indexed by kind · 8 `dyn`/vtable at a cold boundary · 9 process, wasm, or ABI plugin boundary

[LADDER.md](LADDER.md) prices each rung on the three axes, says when to climb to the next, maps every GoF pattern to its cheapest equivalent, and lists the measured residue; open it for every seam you place and every pattern name that comes up.

Done when every seam names its rung and states why the rung below it fails its axis.

### 6. Drill

Get three likely changes from the user; ask rather than invent them. With no user available, take them from the backlog, issues, or decision log and cite the source. For each, list every edit (file and function), its change radius, and the change in interface weight. A drill **passes** when the change lands as new tables, transforms, rows, variants, or spine lines, at the radius the user accepts (zero existing bodies, or the switch sites the compiler lists for a new variant). A drill **fails** otherwise, and the fix is one of two: restructure (split the transform so the new behaviour is a new transform after it) or climb one rung for that seam and run the drill again. Restructure first; it usually costs nothing at runtime.

Done when every drill has a complete edit list and a change radius that is a number, and every failure is fixed or recorded as an exception the user accepted.

### 7. Record

Write one decision record per seam and the drill table, in the formats in [DECISIONS.md](DECISIONS.md), and update `ARCHITECTURE.md` from its template there (spines, ownership map, module DAG, seams, invariants, "adding a feature"). Open DECISIONS.md before writing either; it also has a worked example.

Done when someone who has not read the code can say from `ARCHITECTURE.md` which file a given feature goes in, and every seam has a record.

### 8. Prove

For every seam at rung 5 or above, probe the asm against the concrete version per the `zig` or `rust` skill's `zero-cost.md`. Rung 5 must come back FOLDED, IDENTICAL, EQUIVALENT, or NEGATIVE. Rungs 6 to 9 leave residue by design; the proof is where it sits (per request, per kind, per call, never per element in a hot loop) and the count per run, written in the record. Then make the build enforce the DAG from step 2 (HIERARCHY.md, "The build enforces the DAG").

Done when every rung ≥5 seam's record carries a probe verdict and file, and a dependency cycle or forbidden import fails the build or a test.

## Exit criterion

The work is done when every new seam names its rung, cites a change drill it passes, and has its zero-cost proof (the `zig` or `rust` skill's `zero-cost.md`) for any rung at 5 or above.

## Reviewing an existing codebase

Rebuild steps 1 to 3 from the code first (the data map, the ownership map with each table's real writers, and the spines), because the violations only show up against them. Then check:

- [ ] **owners**: every table is written only from its owning module; list the ones that aren't
- [ ] **direction**: no topic folder (`utils`, `common`, `managers`, `models`) whose files import across owners; no module cycle
- [ ] **build DAG**: the module graph is enforced by Cargo crates, the Zig `build.zig` wiring, or a dependency test, not by review
- [ ] **spine**: one function per pipeline lists every transform; no transform calls another transform or branches on which work to do
- [ ] **files**: each file holds one transformation and opens with an in → out header; leaves never import their module's root
- [ ] **seams**: every seam has two cases today or a drill; a seam with one adapter is speculative and drops to concrete code
- [ ] **rungs**: every seam is on the cheapest rung its axis allows; runtime dispatch never runs per element in a hot loop
- [ ] **enums**: every `switch`/`match` over a boundary enum is exhaustive, with no `else =>` or `_ =>`
- [ ] **handles**: no pointer stored in a struct field; cross-table references are index handles
- [ ] **limits**: every table has a capacity; every transform asserts its entry invariants
- [ ] **drill**: for one change the user names, the edit list and its change radius

Done when every box has a verdict with a file:line, and the drill's change radius is a number.

## Telling an agent what you want

Put this in a prompt or `CLAUDE.md`:

```markdown
Architecture follows `codebase-clean-code`: group by owner and dependency direction,
one spine per pipeline, one owner per table, seams only on a real variation axis and
on the cheapest rung of its ladder. Every new seam names its rung and the change
drill it passes; rung 5 and above ships an asm proof. State the change radius before
writing a feature.
```

## Sources

- Casey Muratori, *Semantic Compression*: caseymuratori.com/blog_0015
- Casey Muratori, *"Clean" Code, Horrible Performance*: computerenhance.com/p/clean-code-horrible-performance
- John Ousterhout, *A Philosophy of Software Design*, ch. 4 "Modules Should Be Deep"
- Michael Feathers, *Working Effectively with Legacy Code* (seams)
- Alex Kladov (matklad), *Push Ifs Up And Fors Down*: matklad.github.io/2023/11/15/push-ifs-up-and-fors-down.html
- Alex Kladov, *ARCHITECTURE.md*: matklad.github.io/2021/02/06/ARCHITECTURE.md.html
- Richard Fabian, *Data-Oriented Design*: dataorienteddesign.com/dodbook
- Schemify `docs/dev/architecture.md` (crates as owners, unifying abstractions, change drill) and `docs/dev/contributing.md` (hard rules)
- ESPice `AGENTS.md` (the build.zig DAG, leaves never import their root); VerA `AGENTS.md` §4 (placement by import, narrow stage outputs, no `else =>` on boundary enums)
