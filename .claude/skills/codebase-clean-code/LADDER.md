# The ladder

Architectural mechanisms ranked by cost, cheapest first. Place each seam on the lowest rung that handles its variation axis, and climb one rung only when a change drill fails at the current one or the "climb when" condition below holds today. The cheapest "pattern" is plain concrete code.

Costs are on the three axes from SKILL.md: **residue** (work left in the hot path next to concrete code), **radius** (existing function bodies edited to add one more case), **weight** (concepts a caller learns).

| # | Mechanism | Residue | Radius for a new case | Weight added | Climb when |
|---|---|---|---|---|---|
| 1 | Concrete code | none (the baseline) | the body itself | 0 | the same code is needed in a second place, or the spine needs to name the step |
| 2 | Free function over plain data | none: FOLDED/IDENTICAL with the hand loop | the one body | 1 signature | two or more cases differ only in numbers or in which rows exist |
| 3 | Table + transform | one indexed load per element (a value table); none for a table per kind | 1 row, or 1 table + 1 transform + 1 spine line; 0 bodies | 1 row type | cases differ in behaviour, not just data |
| 4 | Closed enum + exhaustive switch | per-element dispatch: cheap arms become selects with every arm computed; heavier arms a branch per element (unverified: heavy arms not probed) | 1 arm per switch site, listed by the compiler | 1 enum | the choice is fixed per build or per call site, and the same switch repeats in several transforms |
| 5 | Comptime or generic parameter | none: FOLDED; costs code size per instantiation | 0 bodies: one new instantiation | 1 parameter contract (trait bound, required decls) | the choice is made at runtime, or by someone other than the owner |
| 6 | Request table or command queue | one write and one read per request; a growth call unless capacity is reserved | new emitter: 0 owner bodies; new request kind: 1 variant + 1 arm in the owner's apply | 1 request row type + its ordering rule | the set of kinds is open at runtime and each kind's behaviour is a loop over its batch |
| 7 | Function-pointer table indexed by kind | one indirect call per kind per pass, never per element; callee bodies stay ordinary loops | 1 function + 1 row; 0 bodies | 1 function signature + registration | implementations carry their own state and lifetime and are called a few times per frame |
| 8 | `dyn` / vtable at a cold boundary | one indirect call per call; in an element loop it blocks inlining and vectorization | 1 impl; 0 bodies | 1 trait + object lifetime | the implementation is untrusted, built separately, in another language, or must fail without taking the host down |
| 9 | Process, wasm, or ABI plugin boundary | serialization and a boundary crossing per call (unverified: magnitude not probed) | 1 plugin; 0 core bodies | a versioned ABI, capabilities, snapshot formats | top rung |

## Rung notes

**1. Concrete code.** One case, no drill, no seam. This is the default answer to "should this be abstracted?" and it is the comparison every probe runs against.

**2. Free function over plain data.** A function that takes the tables it reads and writes as parameters. Extract at the second real use, not before (Muratori: compress at the second instance), and only when it meets the extraction rule in HIERARCHY.md.

**3. Table + transform.** Variation by data rows. Two shapes:

- *Kinds differ in numbers*: a value table indexed by kind, `gravity_scale[kind]`. A new kind is a row, and the compiler should refuse a table that lacks it. Zig's `std.EnumArray(Kind, f32).init(.{ ... })` does (`error: missing struct field: feather`). A plain Rust array indexed by `kind as usize` does not, so pin its length next to it: `const _: () = assert!(GRAVITY.len() == Kind::Feather as usize + 1);` fails the build with E0080.
- *Kinds differ in data and behaviour*: a separate table per kind, existence-based (being a drone = having a row in `drones`). A new kind is a table, a transform, and a spine line. No branch on kind exists anywhere.

This is how the expression problem goes away: a new operation is a new transform over existing tables, a new kind is a new table, and both cost zero edited bodies. A sparse fat struct (Fleury's megastruct) is a fine cold-path shortcut while entity counts are small; split it into tables when a profile says to.

**4. Closed enum + exhaustive switch.** For a few kinds that differ a little in behaviour, owned by this module. The switch has no `else =>`/`_ =>`, so a new variant makes the compiler list every site (VerA bans `else =>` on boundary enums by test). When the same switch appears in several transforms, the kind is being dispatched per element in the wrong place: sort or bucket by kind and climb to 3 (a table per kind) or 7.

**5. Comptime or generic parameter.** Strategy resolved at build time: `comptime f: fn (f32) f32`, `anytype`, a monomorphized Rust generic, a trait used only as a generic bound (Schemify's `render::Painter`). Zero residue, but each distinct argument is a new instantiation (about 264 B per comptime constant for a small loop, measured in the `zig` skill's comptime.md). Needs the proof in SKILL.md step 8.

**6. Request table or command queue.** The owner drains a queue of plain rows that other modules appended. It buys explicit order, single ownership, and serializability (undo, logging, RPC, plugins all speak the same rows); it costs a store and a load per request. Reserve capacity at the phase boundary so the emitter loop has no growth call, and bound the queue. Schemify's request tables replace the plugin API, UI actions, JSON-RPC, optimizer write-back, importer output and viewer actions with one mechanism.

**7. Function-pointer table indexed by kind.** For a kind set that is open at runtime (loaded device models, registered commands). Rows are bucketed by kind, and the spine calls `table[kind](batch)` once per kind, so indirect calls scale with the number of kinds, not elements. ESPice's device Batch vtable is this rung: O(device types) indirect calls per evaluation.

**8. `dyn` / vtable at cold boundaries only.** A logger sink, a renderer backend picked at startup, a test fake. Never per element in a hot loop. When the choice is known at build time, rung 5 does the same job at zero residue. Schemify's core has no `dyn`; its only runtime dispatch is the plugin boundary.

**9. Process, wasm, or ABI boundary.** For untrusted, separately built, or other-language code: the plugin talks through snapshots in and request tables out, under capabilities. Schemify's change drills (layout handoff, an AI sizing assistant) land at 0 core bodies because this rung and three request tables existed from M1.

## Measured residue

x86-64-v3, Rust 1.95 `-C opt-level=3 -C codegen-units=1`, Zig 0.17 `-O ReleaseFast`, verdicts from `asm_probe.py`. Static counts are instructions in the listing, not instructions executed. Each probe is the rung written as an exported function beside the concrete loop it replaces.

| Rung | Probe | Rust 1.95 | Zig 0.17 |
|---|---|---|---|
| 2 | free function called from a spine vs hand loop | IDENTICAL (62 instr, 17 vector each) | FOLDED |
| 3 vs 4 | value table `g[kind[i]]` vs per-element `match` on a 2-value enum | NEGATIVE: table 58 instr / 5 branches, match 90 / 10; both vectorized (table by per-lane loads + `vinsertps`, match by `vblendvps`) | NEGATIVE: table 56 instr / 0 vector, switch 68 / 8 vector |
| 4 | `match` whose arms do different work vs arms that pick a value | still vectorized: both arms computed for every element, then blended (100 vs 90 instr) | not probed |
| 5 | comptime fn parameter / generic `Map(T, f)` vs hand loop | FOLDED | FOLDED |
| 5 | `repr(transparent)` newtype vs the bare type | FOLDED | — |
| 6 | push to a `Vec` then drain vs apply directly | RESIDUE: an 8 B store per request and a `grow_one` call when capacity is not reserved | not probed |
| 7 | `TABLE[kind](batch)` over 2 kinds | one indirect call per kind (a `call` and a tail `jmp`); `rock`'s body folded into the vectorized hand loop | one indirect call per kind |
| 8 | `&mut dyn Trait` per element; ctx pointer + fn pointer per element | RESIDUE: one indirect call per element, no vectorization | RESIDUE: one indirect call per element |

Zig 0.17 ships with LLVM's loop vectorizer off, so in this probe the Zig hand loop itself is scalar (0 vector instructions). A rung's Zig verdict says it matches the hand loop; it says nothing about vectorization. 0.18 turns the vectorizer back on, so re-probe the rungs there. A rung that keeps the loop body visible to LLVM (rungs 1–5) can then vectorize with the hand loop. Indirect calls (rungs 7–8) still can't vectorize.

## GoF translation

Each pattern maps to its cheapest equivalent. Use the pattern's name to recognise a need; build the equivalent.

| Pattern | Cheapest equivalent | Rung |
|---|---|---|
| Strategy | comptime parameter or closure passed to a generic | 5 (2 if one strategy) |
| Visitor | enum + exhaustive `switch`/`match` | 4 |
| Observer | request table: subscribers append, the owner drains in spine order | 6 |
| Command | command enum as data; undo and replay walk the rows | 4 + 6 |
| State | enum for the state + transition table `next[state][event]` | 3 + 4 |
| Composite | flat arrays with parent/child indices, walked in index order | 3 |
| Singleton | pass the context explicitly as a parameter | 2 |
| Template Method | the spine, with the varying steps as comptime parameters | 5 |
| Decorator | a transform inserted in the spine before or after the existing one | 3 |
| Adapter | a newtype or wrapper struct (`repr(transparent)` folds) | 5 |
| Factory | a constructor function; one table per kind when kinds differ | 2 or 3 |
| Iterator | the language's `for` over a slice or index range | 1 |

## Sources

- Casey Muratori, *Semantic Compression*: caseymuratori.com/blog_0015
- Old `design-patterns` skill (repo commit 2198aeb), its pattern table and "is the variability axis real today?" rule
- Schemify `docs/dev/architecture.md` § Unifying abstractions, § Change drill
- ESPice `AGENTS.md` § Call conventions
- Ryan Fleury, entity megastruct (via gist.github.com/randyprime/065370cfa73c0dd3cb008eb858e6ba4b)
