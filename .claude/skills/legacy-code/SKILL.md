---
name: legacy-code
description: Safely modify code that lacks tests. Applies the Legacy Code Change Algorithm — find seams, write characterization tests, break dependencies, then change. Use when modifying untested code, adding features to legacy systems, breaking dependencies for testability, wrapping or sprouting new behavior alongside old code, writing characterization tests, or when user mentions "legacy code", "no tests", "untested", "seam", or "safe refactor".
---

# Working Effectively with Legacy Code

> Legacy code = code without tests. No tests means no safety net. No safety net means every change is a guess. — Michael Feathers

## When to Use

- Modifying code that has no tests
- Adding a feature to a codebase you don't fully understand
- Breaking a dependency so code becomes testable
- Wrapping untestable third-party or global-state code
- Writing tests that capture current behavior before changing it
- Any task where the user says "I can't test this" or "I'm afraid to change this"

## The Legacy Code Change Algorithm

Every change to untested code follows this sequence. Do not skip steps.

1. **Identify change points.** Find the exact functions/methods you need to modify. Grep callers.
2. **Find test points.** Find the nearest place you can observe the effect — a return value, a side effect, a written file.
3. **Break dependencies.** Use the minimum technique (see below) to get the code under test. Do NOT refactor broadly — break only what blocks testing.
4. **Write characterization tests.** Test what the code does NOW, not what it should do. Even buggy behavior gets a test.
5. **Make changes and refactor.** Now — and only now — change the code. Your tests catch regressions.

## Seams — Where to Intervene Without Editing

A seam is a place where you can alter behavior without modifying the source file under test.

| Seam Type | How | Use When |
|-----------|-----|----------|
| **Object seam** | Subclass and override the method that causes the dependency | Method calls collaborators you want to fake |
| **Link seam** | Swap the implementation at link/import time (mock module, test double) | Hard-coded imports or linked libraries |
| **Preprocessing seam** | `cfg(test)` / `#[cfg(test)]` / `#ifdef TEST` conditional compilation | Need completely different code path in tests |

Pick the highest seam that works. Object seams are most common and least invasive.

## Dependency-Breaking Techniques

Use the smallest technique that makes the code testable. Ordered from least to most invasive:

1. **Parameterize Constructor/Function** — inject the dependency instead of creating it internally
2. **Sprout Method/Class** — new behavior goes in a new, tested function; old code calls it with one line
3. **Skin and Wrap** — wrap the hard-to-test part (I/O, global state) so you control inputs/outputs
4. **Extract Interface** — extract only the methods the caller uses into a trait/interface

> **DOD tension:** These techniques add abstraction layers (traits, interfaces, vtables). Accept this at module boundaries for testability, but do NOT scatter trait objects through hot loops. Break the dependency at the outer edge, keep inner data processing concrete. If you can sprout a plain function (data in, data out) instead of extracting an interface, do that — no vtable needed.

See [REFERENCE.md](REFERENCE.md) for BAD/GOOD code examples of each technique in Rust, Python, and TypeScript.

## Characterization Tests

Tests that document CURRENT behavior — even if buggy — so you know when you break it.

1. Call the function with a known input
2. Assert something you KNOW is wrong (e.g., `assert result == "xyz"`)
3. Run the test — it fails and shows actual output
4. Replace assertion with actual output — that's your characterization test

**Rules:** Write BEFORE changing anything. Test actual behavior, even if wrong. Cover the paths your change will affect. These are regression tripwires, not specifications.

## Quick Reference

```
Can't test it?
  |
  +--> What's blocking you?
        |
        +-- Hard-coded dependency --> Parameterize Constructor
        +-- Global state / singleton --> Extract and inject
        +-- Giant method, afraid to touch --> Sprout Method
        +-- Third-party API call --> Skin and Wrap
        +-- Need to fake a class --> Extract Interface
        +-- All of the above --> Start with Sprout, iterate
```

## Anti-Patterns

**"I'll refactor first, then add tests"** — Refactoring without tests is changing code without a safety net. Write characterization tests FIRST, even ugly ones.

**"I need to understand all the code before I change it"** — No. Find the seam nearest your change point. Test that seam. Change through that seam. The 10,000 lines around it are irrelevant if your seam holds.

**"One more interface won't hurt"** — Every trait/interface is a real cost (indirection, cognitive load, vtable). Break dependencies at the boundary, not at every layer. One seam at the edge beats five interfaces through the guts.
