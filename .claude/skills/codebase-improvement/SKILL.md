---
name: codebase-improvement
description: Apply the house rules to an existing repository, one block at a time. Use when the user wants a repo audited or brought up to standard, when a codebase predates its conventions, or when an improvement pass needs to cover more code than one context holds.
---

# Codebase improvement

`codebase-rules` says how to write a new module. This applies it to code that already exists, where the two hard problems are different: the repo is larger than one context, and most of it should be left alone.

The unit of work is a **block** — one transformation with a boundary, not one file. A block is small enough to hold in context, verifiable on its own, and lands as one commit.

## 1. Inventory

Read before touching anything. Produce a table of every block with: entry points, roughly how many lines, whether it has tests, and whether it is on a hot path.

"Hot" is a claim that needs evidence — a profile, a benchmark, or a call site in a loop over many elements. A block nobody profiled is **cold until proven otherwise**, and that default is what keeps this skill from becoming a repo-wide rewrite.

Then rebuild the architecture the blocks sit in, per `codebase-clean-code`: the owner of every table, the spine of every pipeline, and the module dependency graph. Flag every topic folder (`utils`, `models`, `common`), every import cycle, and every table written by more than one module. Note each block's domain skill where it has one: `spice-engine`, `compiler-construction`, `gui-architecture`.

Done when every source file belongs to exactly one block, each block has a hot/cold verdict with its reason, the owner map and spines are written down, and every flag is attached to the block it lives in.

## 2. Rank

Cold blocks and hot blocks get different treatment, and the asymmetry is the point:

| Block | Gets |
|---|---|
| Cold, tested, documented | **nothing** — say so and move on |
| Cold, untested | tests at agreed seams (`tdd`) |
| Cold, undocumented public API | the doc audit (`doc-comments`) |
| Hot, with a measurement | the full chain from `codebase-rules` |
| Hot, without a measurement | a benchmark first — then re-rank |
| Carries an architecture flag from step 1 | the seam fix (step 4.6), hot or cold |

Most blocks in most repos land in the first two rows. A ranking where most blocks need the full chain is a ranking that skipped step 1's evidence requirement.

Done when every block has a row, and the count of full-chain blocks is small enough to name.

## 3. Write down the plan, then confirm it

The ranked table gets written down before anything is edited, always. What happens next depends on who is running:

- **With a user present** — show the table and ask which blocks to take and in what order. A user who wants all of them still gets asked, because the ranking is where a wrong hot/cold call is cheapest to catch.
- **Running autonomously**, as a subagent or scheduled job with nobody to ask — the table becomes the commitment instead of the request. Work only the blocks in it, and open the report with it so the caller can see what the ranking claimed before the diff is read.

Either way the table is what bounds the pass. Blocks that appear in a diff but not in the table are scope that escaped, and they are the failure this step exists to prevent.

Done when the table exists in writing and the block list is fixed.

## 4. Per block

Each block runs independently, start to finish, before the next one begins. Blocks do not share a commit and do not share a review.

1. **Delete first.** Before improving a block, establish it should exist. Code reachable from nothing, a fixture no test loads, a wrapper whose body is one call, an abstraction that was never repeated — deletion is the change with no bugs in it, and it is the one that pays most per minute spent. `ponytail-review` for the judgment, `ponytail-audit` for a systematic sweep. Verify unreachability rather than inferring it: grep every identifier, check the build graph, and say which of the two you did.
2. **Then shrink.** Whatever survives, ask what it looks like with less code — stdlib in place of a hand-rolled helper, one shared function where callers each restated the same invariant, a language feature in place of the machinery around it. A block that ends larger than it started needs a reason stated in the report.
3. **Characterize** — the six questions from `data-oriented-design` against what remains. What data in, what out, how many, how wide, what access pattern, what lifetime. Where the block has a domain skill, check it against that skill's rules too.
4. **Pin the behaviour** — tests at the seams *before* any change that isn't a deletion, so the refactor has an oracle. A block with no tests gets them here or gets skipped; there is no third option.
5. **Layout** — the change that pays most, per `data-oriented-design`. Land it alone.
6. **Seams** — every seam in the block gets its ladder rung and a change drill (`codebase-clean-code`); a seam that fails its drill moves to the rung that passes, and a seam with no second case and no drill becomes concrete code. Step 1's flags are fixed here: a second writer becomes a request table, a cycle is broken by moving the file to the owner it imports. Any seam at rung 5 or above on a hot path gets a codegen-probe verdict wired into the test step (the `zig` / `rust` skill's `zero-cost.md`).
7. **Language and contracts** — the `zig` or `rust` skill's idioms and memory discipline (`migration-0.17.md` for Zig 0.16 code), then `assertions`: a stated capacity for every table, a bound on every loop and queue, two asserts per function on average.
8. **Branches, then lanes** — only on a hot block, only with a benchmark. `branchless`, then `simd-loops`, then a lane axis (the `zig` / `rust` skill's `simd-first.md`) at a wall. Keep the scalar version.
9. **Docs** — the `doc-comments` audit over the block's public surface.
10. **Verify** — tests pass, and for a hot block, the benchmark number before and after. A change that doesn't move the number gets reverted, and reporting that is a complete result.

Steps 1 and 2 come first because they change what the later steps have to cover. Documenting, testing, or vectorizing a block that should have been deleted is work that has to be undone.

Never combine a behaviour change with a layout change in one commit. When the tests go red afterwards, that separation is the difference between a bisect and a guess.

Done when the block's tests pass, every seam names its rung and drill, its public items are documented, and any performance claim has a number attached.

## 5. Report

Open with the step 3 table, so the plan and the diff can be compared. Then per block: what was deleted, what changed, what was deliberately left, and the numbers. Blocks skipped in step 2 appear here too — a list of what was examined and left alone is the evidence that the pass was scoped rather than partial.

Lines deleted is the headline figure. A pass with a negative net line count did the job; a pass that only added needs to say what it bought.

Done when every block from step 1 appears in the report with a disposition, and any file touched outside the table is named as such.

## Scaling past one context

For a repo with more blocks than one session can hold, this fans out: step 1 and 2 are a read-only sweep, step 4 is independent per block, and blocks that touch disjoint files can run concurrently in worktrees.

That is a `Workflow` shape, and it needs the user's explicit opt-in before launching — it spawns an agent per block and the cost scales with the block count. Ask, name the number of blocks, then run it. Steps 1–3 still happen first and still happen once; the fan-out only ever covers step 4.

## What this skill will not do

Improvement means the code does the same thing, better. A block whose *behaviour* is wrong is a bug and belongs to `diagnosing-bugs`; a block that should not exist belongs to `ponytail-review`. Rewriting either one under an improvement pass hides a real change inside a diff nobody reads closely.
