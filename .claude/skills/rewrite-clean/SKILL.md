---
name: rewrite-clean
description: Rewrite an existing project through the four-phase methodology, one slice at a time, with the old code kept alive as the behavioral oracle.
disable-model-invocation: true
---

Two things are true at once in a rewrite, and the whole method comes from holding both: the **structure** is replaced, the **behavior** is not. The old code is the **oracle** — the only complete specification of what this project actually does, bugs included. It stays runnable and callable until the code replacing it has been diffed against it and passed.

That is what separates this from a greenfield build, and from the second-system rewrite that runs for six months and ships something quietly missing a third of the original behavior. Nobody wrote that third down. Only the old code knows it.

The four phases, the vocabulary (module, interface, seam, adapter), and the reasoning behind each rule come from `~/Documents/Projects/OmarSiwy.github.io/Blogs/Writing_Clean_Code.md`. Read it once at the start. This skill adds what a rewrite needs on top: the oracle, the slicing, and the gate at the end of every phase.

## Inventory the project

You cannot plan an architecture for behavior you have not found yet. Produce one list covering:

- Every entry point — CLI flag, HTTP route, exported symbol, event handler, cron.
- Every piece of persistent state — file format, DB table, global, on-disk cache.
- Every external dependency and what it is actually used for.

Mark each row **live**, **dead**, or **unknown**. `unknown` is the productive bucket: it is behavior that exists and nobody can say whether it matters.

Done when every entry point is on the list and classified. Resist the urge to start designing while the list is still growing — an architecture drawn over a partial inventory gets redrawn.

## Cut the slices

A **slice** is one coarse module with one seam to the rest of the program: the unit that gets rewritten, tested, and flipped as a whole. Cut on the seams that already exist, and order the slices leaf-first, fewest dependencies on not-yet-rewritten code.

The size rule is blunt on purpose: **a slice you cannot carry through all four phases in one sitting is two slices.** A rewrite fails by being too large before it fails for any other reason.

Done when you have an ordered list of slices, each with its seam named and its callers listed, and every inventory row assigned to exactly one slice. A row belonging to no slice is behavior about to be dropped silently.

## Pin the oracle

Before rewriting anything, make the old code answerable. Tag or branch the current state, then build the differential harness: a runner that feeds identical input to old and new and diffs the output.

Non-determinism has to be injected at the seam — clock, RNG, iteration order, environment — so both sides see the same values. Whatever cannot be injected has to be excluded from the diff explicitly, and written down as excluded.

Then run the harness **old against old** and get zero diffs. That green run is the whole point of this step: it proves the harness itself is sound, so that every later diff means the new code, not the harness. Without it, the first red diff is ambiguous and you will spend a day deciding whether to trust your own tooling.

Done when old-vs-old is green on a recorded input corpus drawn from real usage, not from invented examples.

## Keep the ledger

One running file, `REWRITE.md`: every intentional behavior change, with the old behavior, the new behavior, and why.

The rule it exists to enforce: **a differential diff is either in the ledger or it is a bug.** No exceptions, because the moment a diff can be waved off as "probably the old bug" the oracle stops meaning anything.

This includes bugs. A bug you find in the old code gets logged, not fixed — fixing it mid-rewrite silently changes the oracle underneath you. Fix it after the slice flips, as its own change, where it gets its own test.

## Run the four phases, per slice

Each phase ends on a gate. The gate is the completion criterion; a phase is not done because the work feels done.

**Plan-Phase** — `/grilling` on this slice's modules, seams, and interfaces, plus the testing requirements and which seams get compiler-gated test adapters. The rewrite-specific material for the grilling is the inventory rows for this slice: grill on which observed behaviors are load-bearing and which merely leaked out of an implementation detail. Only the user knows, and asking now is far cheaper than discovering it from a red diff later.

> Gate: the interface is written down, and every inventory row for this slice is classified preserve / drop / ledger.

**Definition-Phase** — `/data-oriented-design`, `/humanizer`, `/grilling`, `/api-design`. Freeze the structs and signatures; write no bodies and no tests.

This is where a rewrite earns most of its value, so it is also where the strongest pull backwards lives. The old data structures are a hypothesis, not a constraint. Answer the six DOD questions from the spec, not from the old struct — the field widths, the AoS layout, and the pointer graph in the old code are the specific things you came here to fix, and copying them forward is how a rewrite ends up as the same program in a new syntax.

> Gate: every stub compiles with no bodies, and no old type name appears in a new signature.

**Testing-Phase** — implement the test plan inherited from the Plan-Phase against the frozen signatures, and wire the differential harness to the new seam.

> Gate: the tests compile and **fail** against the stubs. Red-on-stubs is the check that matters — a suite that passes against unimplemented stubs is exercising nothing, and it will pass against a wrong implementation too.

**Implementation-Phase** — `/ponytail` per stub, with debug asserts on inputs and intermediate stages. Bulk loops go through the utility combinators; the body handed to a combinator carries no data-dependent branch (`/branchless`, then `/simd-loops`). Give the agent real knowledge of the libraries in use rather than letting it guess.

> Gate: the slice's tests are green, the harness is green against the oracle on the recorded corpus, and every remaining diff has a ledger entry.

## Flip and retire

**Strangler**, not big-bang. The seam routes to the new implementation; call sites flip one at a time, and the old path stays callable behind a flag for one release.

Delete the old slice last. Deleting it retires that piece of the oracle, so it happens after the flag has been off under real usage — not the moment the tests go green.

Done when the old slice is gone, its inventory rows are all preserved or ledgered, and the next slice's seam is the only place the two generations still touch.

## Before you start: should this be a rewrite at all

Two outs worth taking, both cheaper than the work below them:

- **The inventory came back mostly dead.** Deleting entry points is a smaller diff than rewriting them, and it shrinks every slice that survives. Do the deleting first, then re-inventory.
- **A slice goes green with the old code merely reorganized.** That is a refactor, and it just cost you a fraction of a rewrite. Take it and move to the next slice — the goal is the end state, not the rewriting.
