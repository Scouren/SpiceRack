---
name: codebase-rules
description: House rules and build order for performance-sensitive systems code. Use when starting a module in a project that references these rules, when two engineering skills give conflicting guidance, or when the user asks how this codebase is meant to be written.
---

# Codebase rules

Every skill below is loaded and reachable on its own. This one carries the two things none of them can: the **order**, and the **rulings** where they contradict each other.

## Before the first line

**Where does the boundary go** → `api-design`. Muratori's rules, semantic compression, and the reusable-component checklist. Getting this wrong is the only mistake on this page that a later step cannot fix.

**Where each piece lives** → `codebase-clean-code`. Owners, spines and seams, each seam priced on its ladder rung and checked by a change drill.

**What is the data** → `data-oriented-design`, the Six Questions. Answer all six out loud. Question 3 (how wide is each field, from the spec) is the one that gets skipped and the one that pays twice — narrow fields mean more entities per cache line *and* more lanes per vector.

## While writing

**The language** → `zig` or `rust`: idioms, memory discipline, and the codegen probe that admits an abstraction.

**Contracts** → `assertions`. Capacities, bounded loops, two asserts per function on average.

**The loop** → `tdd`. Red before green, one vertical slice at a time. Tests are not a phase that follows implementation; a horizontal slice of tests written up front verifies imagined behaviour. Confirm the seams with the user before writing the first test.

**A data-dependent branch in a hot loop** → `branchless` first, `simd-loops` second. That order is not preference: a data-dependent branch is what stops lanes from being independent, so it blocks vectorization outright.

**A hot loop over many elements** → `simd-loops`. Triage before writing anything — element type, count, and whether the dependency is none, accumulator, or chain. A chain, or an output length that depends on the data, is a **wall**: look for a lane axis across independent instances first — the `zig` or `rust` skill's `simd-first.md`.

**Where vectors may live** → `data-oriented-design/zig.md` §9. Layout is the decision, `@Vector` is the loop detail; it never reaches a struct field or a public signature.

## After

**The doc audit** → `doc-comments`. Run it once the feature is written, so the comments describe what shipped rather than what was intended. Every public item names the fact its comment carries; grep the bodies for `assert` and allocation to catch the contracts added after the comment was.

**The review** → `code-review`, then `ponytail-review` for what should not have been built at all.

## Rulings

**Clean Code loses to the data, in hot code only.** "Extract till you drop" and "a function should do one thing" directly contradict caller-owns-the-loop and no-hidden-control-flow. Where the code is hot, the data layout wins and the small-function instinct is the thing that gets dropped. Everywhere else — and that is most code — Clean Code's naming and function-size guidance is fine and cheaper to follow than to argue with. The tell for which regime you are in is a measurement, not a taste.

**Pure by default; impure when the pieces are testable.** Prefer a pure function because it is testable in isolation. Accept an impure one when every child it calls is independently tested and a larger test covers the composition. State which of the two you are doing — the failure is drifting into impure because it was easier, then discovering nothing underneath was covered.

**Testability is a seam question, not a purity question.** `tdd` already rules on this: tests go at pre-agreed public seams. Purity is one way to get a seam, not the definition of one.

**Measure before you accept any of the performance rungs.** `data-oriented-design/measurement.md` holds the profile-first workflow. A vectorized loop that benchmarks slower is a normal outcome, and reporting the number while keeping the scalar version is a complete result.

## Adopting this in a project

Paste into the project's `CLAUDE.md` or `AGENTS.md`:

```markdown
## How this codebase is written

Follow the `codebase-rules` skill — it carries the build order and the rulings
where the engineering skills disagree. Read it before starting a new module.

Project specifics that override it: <compiler pin, target CPU baseline, any
house convention that differs>.
```

Keep the override list short and factual. Anything longer than a few lines is a
rule that belongs in a skill, not in a per-project file where it goes stale
unnoticed.

Applying this to an existing repo rather than a new module is the
`codebase-improvement` skill's job.
