---
name: assertions
description: Assertions as executable contracts for programmer errors, after TigerBeetle's TIGER_STYLE. Use when writing or reviewing a function in Zig or Rust, when deciding between an assert and an error return, when giving a table a capacity or a loop a bound, or when an up-front assert could remove bounds checks from a hot loop.
---

# Assertions

An assertion is an executable **contract**: a precondition on entry, a postcondition on exit, an invariant in between. It exists to catch a **programmer error**, a state the code's own logic says cannot happen. When one fires, the only correct response is to crash: a correctness bug becomes a liveness bug, and every fuzz run and test becomes a contract check (TIGER_STYLE).

An **operating error** is the other kind: bad input, a missing file, a full disk, out of memory. It is expected, so it is handled with an error return. The classification decides everything below, so it comes first.

Density target, from TIGER_STYLE and Schemify's `contributing.md` rule 5: at least **two asserts per function on average**, covering the entry and at least one exit invariant, in both the positive and the **negative space**.

Language mechanics (what each assert compiles to in each build mode, and what a false one does) are in [zig.md](zig.md) and [rust.md](rust.md). Open the one for the language you are writing before relying on an assert in a release build.

## Writing a function's asserts

### 1. Classify every check

For each condition the function tests, ask where a violation would come from.

| Source of the bad value | Kind | Write |
|---|---|---|
| The caller broke a documented precondition | programmer error | assert |
| This function's own logic (a postcondition, a loop invariant) | programmer error | assert |
| A file, socket, user, CLI flag, plugin, or allocator | operating error | error return, handled by the caller |

Input from a trust boundary is validated with an error return once, at the boundary; past that point the validated property is a precondition and gets asserted. The boundary function owns the error; every function behind it owns an assert.

Done when every check in the function is classified as one of the two kinds, and no assert tests a value that came straight from outside the program.

### 2. Assert the entry and the exit

Assert every argument property the body relies on: lengths, ranges, non-overlap, sortedness, a handle's generation. Then assert at least one property of the result or of the state left behind: the count written, the length after a push, the sum that must be preserved. "A function must not operate blindly on data it has not checked" (TIGER_STYLE).

Keep each assert's cost below the work it guards: an unconditional O(1) assert before an O(N) loop; an O(N) check of an O(1) operation goes behind a comptime verification flag (TigerBeetle's `constants.verify`). Slow asserts cut how many randomized tests run, which cuts coverage.

Every assert reachable from a public entry is a precondition the caller must know about; it goes into the doc comment (`Asserts that ...` in Zig, `# Panics` in Rust). That line is the `doc-comments` skill's rank 1.

Done when the function has an entry assert for each property the body relies on and at least one exit assert, and each reachable assert appears in the doc comment.

### 3. Pair it across two code paths

For each property worth enforcing, find a second code path that can check it too (TigerBeetle, "It takes two to contract"). The pair sits on opposite sides of a boundary the data crosses:

- before a write and after the read that brings it back (disk, network, a serialized buffer);
- in the producer that appends a row and in the consumer that drains it;
- in `encode` and in `decode`; in `alloc` and in `free`; in `push` and in `pop`.

A pair catches the bug that a single assert cannot: corruption in between, and two sides that disagree on the format.

Done when every property you asserted at a boundary has its partner on the other side, or a one-line `//` note naming why no second path exists.

### 4. Cover the negative space

Assert what must be false as well as what must be true. The boundary between valid and invalid is where off-by-one and wrong-branch bugs live.

- Both arms of a classification: `if (valid) assert(handle < len)` and `if (!valid) assert(handle >= len)`.
- An implication as a one-line `if`: `if (a) assert(b);`.
- The state that must not exist: after `free`, assert the slot is on the free list; after `clear`, assert `count == 0`.

Done when every branch that splits the input into valid and invalid has an assert on each side, or the else side is unreachable by type.

### 5. Split compound asserts

Write `assert(a); assert(b);`, never `assert(a and b)`. The panic site then names the condition that failed. A compound condition reads as one fact and hides which half was wrong.

The split form costs nothing in the loop it guards: probed in both languages, the loop is instruction-for-instruction the same as the compound form's. The differences sit outside the loop: in Zig the frame-pointer push moves from the cold panic path to the entry (one static instruction), in Rust each assert gets its own panic call site (numbers in the language files).

Done when no assert in the function joins two conditions with `and` / `&&`. A relational check written as one comparison (`a.len == b.len`) is one condition.

### 6. Assert the constants at compile time

Relationships between compile-time constants are checked once, at build time, for free: header size against block size, a pipeline depth against a batch size, a struct's size against the wire format, a power of two the code masks with. Zig: `comptime assert(...)`. Rust: `const _: () = assert!(...);`. A failure is a compile error naming the line.

Done when every constant the code derives another constant from, masks with, or divides by has its relationship asserted at compile time.

### 7. Put a bound on everything

Every table has a **capacity**, a named constant with a unit (`lines_max`, `queue_capacity`), and the code asserts against it. Every loop and every queue has a fixed upper bound, so a bug terminates as a crash instead of a hang or a tail-latency spike:

- a `while` loop that is not a `for` over a slice carries an iteration counter and asserts it stays under the bound;
- a queue asserts `count < capacity` before a push, and the push path returns an operating error when the capacity is a user-visible limit;
- a loop meant never to terminate (an event loop) asserts that, so a fall-through is a crash.

Done when every table in the change names its capacity, and every loop either iterates a finite slice or range or asserts an explicit bound.

## Negative cost: the up-front length assert

An assert is a check, and checks are usually a cost. One case runs the other way: in a safe build (Zig ReleaseSafe, any Rust build), an indexed loop that **stores** through several slices carries a bounds check per slice per iteration, and those checks can stop the loop from being unrolled. One length assert before the loop proves every index in range, so the per-iteration checks disappear and the loop body becomes clean.

- Zig 0.17 ReleaseSafe: no assert gives two bounds checks per iteration and a rolled loop; `assert(b.len >= a.len); assert(o.len >= a.len);` up front gives an 8×-unrolled loop with no checks inside. Probed.
- Rust 1.95 opt-level 3: LLVM already splits the loop at the shortest length and vectorizes the safe prefix; the assert removes the bounds checks from the scalar tail and replaces two bounds-check panic sites with one assert panic; the main vector loop is the same. Probed.
- A read-only indexed loop gets its check hoisted without any assert in both languages, so the assert buys one to three static instructions there; report it as that.
- An assert does not remove other per-element checks: with Zig's checked `+` instead of `+%` the overflow check still runs per element and the loop stays rolled.

The probe, the four variants, and the numbers are in [zig.md](zig.md) and [rust.md](rust.md). The general zero-cost protocol (how to run `asm_probe.py`, what each verdict means, when a codegen claim needs a benchmark) is the `zig` skill's `zero-cost.md` and the `rust` skill's `zero-cost.md`; follow it before writing "this assert makes the loop faster" in a comment or a PR.

Done when every hot indexed loop over two or more slices has its length relationship asserted up front, and any performance claim about it cites a probe verdict.

## Auditing a file

1. Count functions and asserts in the files the change touched: `grep -c 'assert' file` against `grep -cE '^\s*(pub )?fn ' file`. Below two per function on average, read the low-density functions first.
2. For each function, walk steps 1–7 and add what is missing.
3. Run the test suite in a safe build (Zig Debug or ReleaseSafe; Rust with debug assertions on), so every assert executes.

Done when the touched files average two or more asserts per function, every step's criterion holds for each touched function, and the tests pass in a safe build.

Sources: TigerBeetle `docs/TIGER_STYLE.md` (Safety); zigcc-skills `zig-tiger-style`; Schemify `docs/dev/contributing.md` rule 5; probes in this skill's language files.
