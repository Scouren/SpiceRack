# Pseudocode Artifact Specification

A clean-room pseudocode artifact is the *only* information that crosses the barrier, so it must be complete enough to reimplement the function from scratch — and clean enough that the result is a genuine reimplementation, not a transcription. This file defines both edges.

## The artifact must fully capture behavior

For each function, the artifact specifies:

- **Purpose** — one line: what the function is for.
- **Inputs** — each parameter: meaning, type *concept* (not language type), valid range, units, ownership/mutability.
- **Outputs / return** — what is produced and its meaning.
- **Side effects** — I/O, mutation of shared state, allocation, logging, anything observable beyond the return value. Omitting these is the most common way a reimplementation silently diverges.
- **Preconditions / invariants** — what must hold on entry, what the function guarantees on exit.
- **Error handling** — every failure mode, what triggers it, and how it is signaled (exception, error code, sentinel). State the *behavior*, not the original's exact exception type or message.
- **Edge cases** — empty input, zero, overflow, null/absent values, boundary indices, concurrency assumptions.
- **Algorithm** — the control flow and data transformations as numbered plain-language steps. Name abstract data structures by role ("a last-in-first-out stack of pending nodes"), not by the source's concrete type or variable names.
- **Complexity / performance intent** — if the original deliberately uses a particular complexity class or avoids a costly operation, say so, because a naive reimplementation might regress it.
- **Dependencies / collaborators** — other units this function calls, described by contract, so the Implementer knows the interface it can rely on.

## The artifact must exclude original expression

The barrier only works if the pseudocode carries behavior, not the source's incidental wording. Exclude:

- Verbatim lines of source code.
- Copied comments or docstrings.
- The original's exact identifier names where they are arbitrary (rename to role-descriptive terms). Keep names only where they are part of a public contract the reimplementation must match (e.g. a wire-protocol field, a public API parameter the caller depends on).
- Formatting, file layout, or ordering that is incidental to behavior.
- Micro-optimizations that are implementation detail rather than required behavior — unless they are behavior (see "complexity intent").

The test: *if two competent engineers each implemented from this artifact, their code would behave identically but need not look alike.* If the artifact would force them to reproduce the original's exact structure, it is leaking expression — abstract it further. If it leaves behavior ambiguous, it is underspecified — add detail.

## Worked example

Source function (conceptually): deduplicates a list while preserving first-seen order.

Good artifact:
```
Purpose: Return the input items with duplicates removed, keeping the order of first appearance.

Inputs:
- items: an ordered sequence of comparable elements. May be empty. May contain duplicates.

Output:
- A new ordered sequence containing each distinct element once, in the order it first appeared in the input.

Side effects: none. The input is not modified.

Errors: none for valid sequences. If an element is not comparable for equality, behavior is the language's natural equality failure (do not invent handling the original did not have).

Edge cases:
- Empty input -> empty output.
- All-identical input -> single-element output.

Algorithm:
1. Maintain a set of elements already seen and an ordered result list.
2. For each element in input order:
   a. If it is already in the seen-set, skip it.
   b. Otherwise add it to the seen-set and append it to the result.
3. Return the result.

Complexity intent: linear in the number of items; uses a membership set to avoid quadratic scanning.
```

Note what is absent: no original variable names, no language types, no source comments. Note what is present: the order guarantee, the no-mutation guarantee, the linear-time intent. An Implementer could write this correctly in any language — using pony-tail's style — without ever seeing the source.

## Checklist before marking a function `described`

- Could someone implement this with zero access to the source?
- Are all side effects and error modes listed?
- Are edge cases enumerated, not assumed?
- Have arbitrary original identifiers been renamed to roles?
- Is any required behavior (ordering, complexity, precision) stated explicitly rather than left implicit in copied structure?
