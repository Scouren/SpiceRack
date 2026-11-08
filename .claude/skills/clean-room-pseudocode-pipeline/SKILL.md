---
name: clean-room-pseudocode-pipeline
description: Reimplement a codebase function-by-function through a strict clean-room pseudocode handoff. A Describer role reads the real source and writes language-agnostic, behavior-complete pseudocode; an Implementer role reads ONLY that pseudocode, never the original source, and writes the target-language implementation using the pony-tail code-writing skill. Use when the user wants to port, rewrite, translate, or reimplement a codebase, architecture, or module into another language; wants a clean-room or Chinese-wall reimplementation; wants every component and every function covered systematically with a coverage ledger; or asks to describe code as pseudocode and have a separate agent implement it without seeing the original. Requires a separate pony-tail skill installed for the implementation step.
---

# Clean-Room Pseudocode Pipeline

Reimplement a codebase by passing it through a deliberate information barrier. One role reads the real source and distills each function into language-agnostic pseudocode. A second role reads **only** that pseudocode — never the original source — and writes the target-language implementation. The barrier is the whole point: it forces the pseudocode to be a complete, self-sufficient specification, and it keeps the new implementation's provenance traceable to the spec rather than to copied source.

This works whether the two roles are two separate agent sessions, two subagents, or the same agent wearing each hat in turn — as long as the barrier is real (see below).

**Scope note.** Use this on code you have the right to reimplement — your own code, code you are porting, code you are licensed to rebuild, or code you are studying. The clean-room barrier here is a discipline for spec completeness and provenance, not a tool for stripping copyright off third-party proprietary code.

## Why the barrier matters

If the Implementer can peek at the original source, two failures creep in. First, the pseudocode stops being trustworthy — gaps in it get silently filled from the source, so you never learn the spec was incomplete. Second, the new code starts mirroring the original's expression (identifier choices, structure, idioms) instead of being a genuine reimplementation. Keeping the wall intact is what makes the output both correct *and* independently derived.

## The pipeline

Run these phases in order. Do not let the Implementer start before the barrier is established.

### Phase 0 — Inventory (Describer)
Walk the codebase and build a **coverage ledger**: every architectural unit (module / package / component / class) and, under each, every function or method. This is the checklist that guarantees "every architecture, every function" actually gets covered and nothing is silently dropped. Use the bundled script:

```bash
python scripts/coverage_ledger.py init --root <path-to-source> --out ledger.json
```

It scans the tree and emits a ledger of units and functions, each marked `pending`. Review it with the user, prune anything out of scope, and confirm the target language before proceeding.

### Phase 1 — Describe (Describer, has source access)
Go through the ledger unit by unit, function by function. For each function, write one self-contained pseudocode artifact that fully specifies its behavior — inputs, outputs, side effects, error handling, edge cases, invariants, and the algorithm in plain steps — **without carrying over verbatim source, copied comments, or incidental implementation detail**. The exact contents and the line a good artifact draws between "behavior" and "copied expression" are in `references/pseudocode-spec.md`. Mark each function `described` in the ledger as you finish it.

Write each artifact to its own file, e.g. `pseudocode/<unit>/<function>.md`. These files are the *only* thing that crosses the barrier.

### Phase 2 — Establish the barrier
Before the Implementer starts, make the wall real. The Implementer's working context must contain the pseudocode artifacts and nothing from the original source tree:
- Separate session/subagent: give it only the `pseudocode/` directory; do not provide the source path.
- Same agent, sequential: when switching to the Implementer hat, stop reading source files entirely and work solely from the artifacts. Do not quote, recall, or reference the original code.

The information-barrier rules and how to handle questions that arise mid-implementation are in `references/handoff-protocol.md`.

### Phase 3 — Implement (Implementer, pseudocode-only)
For each pseudocode artifact, write the target-language implementation. **Invoke the `pony-tail` skill to do the actual code-writing** — that skill governs how the code should be written (style, structure, quality discipline) in the chosen language. This skill owns the *pipeline and the barrier*; `pony-tail` owns *how each function is coded*. Mark each function `implemented` in the ledger.

If `pony-tail` is not installed, stop and tell the user it is required for this phase rather than improvising a substitute house style.

### Phase 4 — Verify without breaking the wall
Confirm the reimplementation matches the intended behavior using only barrier-safe evidence — tests and contracts derived from the pseudocode, not a side-by-side diff against the original source. The reconciliation options (including a third role that *is* allowed to see both sides to triage mismatches) are in `references/handoff-protocol.md`. Mark each function `verified`.

## Coverage is the success criterion

The user asked for *every* architecture and *every* function. Completeness, not cleverness, is how this skill is judged. Keep the ledger current and, at the end, report counts per status so any `pending` or `described`-but-not-`implemented` items are visible. The script summarizes:

```bash
python scripts/coverage_ledger.py status --ledger ledger.json
```

## Reference files

- `references/pseudocode-spec.md` — What a clean-room pseudocode artifact must contain and must exclude, with a worked example and a checklist.
- `references/handoff-protocol.md` — The information-barrier rules, the role definitions, how to handle Implementer questions, and verification/reconciliation without leaking source.
