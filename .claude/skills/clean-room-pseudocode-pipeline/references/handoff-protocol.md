# Handoff Protocol and Information Barrier

This file defines the roles, the rules that keep the barrier real, and how to verify the result without leaking the original source.

## The three roles

- **Describer** — has full access to the original source. Produces pseudocode artifacts (per `references/pseudocode-spec.md`) and maintains the coverage ledger. Never writes target-language code.
- **Implementer** — has access to the pseudocode artifacts and the target language only. Never sees, quotes, or recalls the original source. Invokes the `pony-tail` skill to write each implementation. This is the role behind the wall.
- **Reconciler** (optional, used only in Phase 4) — the one role permitted to see both the original and the reimplementation, *after* implementation, purely to diagnose behavioral mismatches. The Reconciler never feeds source back to the Implementer; it files corrected or clarified pseudocode, and the Implementer reworks from that.

When one agent plays multiple roles sequentially, the role boundary is a context boundary: on switching to Implementer, stop reading source files and rely only on artifacts; do not let remembered source guide the code.

## Rules that keep the barrier real

1. **Only pseudocode artifacts cross from Describe to Implement.** Not source, not diffs, not "just this one tricky line."
2. **The Implementer's questions go back to the Describer, not to the source.** If an artifact is ambiguous, the Implementer raises it; the Describer answers by *revising the pseudocode* (consulting the source as needed). The fix lives in the artifact, which also repairs the spec for anyone else.
3. **No source identifiers leak through questions or answers.** Clarifications are phrased in behavioral terms.
4. **Verification evidence must be barrier-safe.** Tests and contracts are written from the pseudocode, not transcribed from the original's test suite if that suite would reveal source structure. (Behavioral test *cases* derived from documented behavior are fine; copying the original's test code is not.)

## Handling Implementer questions

When the Implementer hits ambiguity:
- It writes the question against the artifact, e.g. "Artifact says 'skip duplicates' — is equality by value or by identity?"
- The Describer resolves it by editing the artifact (e.g. adding "equality is by value").
- The Implementer re-reads the updated artifact and continues.

This loop is the mechanism that turns an incomplete spec into a complete one. Treat every Implementer question as a defect in the pseudocode to be fixed at the source of truth — the artifact.

## Verification without breaking the wall

The goal is to show the reimplementation behaves like the intended spec, using evidence that does not require the Implementer to see the source.

Options, in rough order of strength:

1. **Spec-derived tests.** Author tests from the pseudocode's stated behavior, inputs, outputs, edge cases, and error modes. Run them against the new implementation. These can be written by the Describer (who knows intended behavior) and run by the Implementer.
2. **Differential testing through a neutral harness.** If you may run the original, feed identical inputs to both the original and the reimplementation and compare outputs — *without showing the Implementer the original's code*. The harness compares results; the Implementer only sees pass/fail and failing inputs, then fixes against a clarified artifact.
3. **Reconciler triage.** For mismatches the above cannot explain, the Reconciler inspects both sides, identifies which behavior the artifact failed to capture, and updates the artifact. The Implementer reworks from the corrected artifact — never from the source.

Mark a function `verified` only when its spec-derived tests pass (and differential tests, if used, agree).

## End-of-run report

Before declaring done, report the ledger status counts and list anything not `verified`. A function stuck at `described` means the barrier held but implementation is incomplete; a function at `implemented` but not `verified` means behavior is unconfirmed. Completeness across every unit and every function is the success criterion.
