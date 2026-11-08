---
name: long-term-garbage-collection
description: Garbage-collect Rust and Zig build caches and queue builds on a RAM- or disk-constrained machine. Use when several agents or worktrees trigger overlapping builds, duplicated caches, or memory exhaustion; when `target/`, `.zig-cache`, `zig-pkg`, or sccache directories keep growing; or when the user mentions a build queue or cache budgets.
---

# Long-term garbage collection

Separate coding concurrency from build concurrency: many agents edit, one machine-wide **queue** compiles, and a **collector** keeps what builds leave behind inside a **budget**. Treat RAM and disk as the heap: builds allocate, warm caches are the live set, and collection is **stop-the-world**.

Before the first build, read the toolchain file for every language in play: [rust.md](rust.md) for Cargo and sccache, [zig.md](zig.md) for the Zig caches and native GC status.

## Survey

Read the project instructions, existing build wrappers, toolchain versions, free RAM and disk, and builds already running. Keep the project's required validation and settings; this skill changes how builds run, not what they must prove.

Done when you can name the queue (or its absence), each toolchain version, free RAM and disk, and every active build.

## The queue

Route every heavy check, build, link, and test through **one machine-wide queue**. Reuse an existing queue; otherwise set up a shared process lock or a designated build coordinator, with one queue identity for every agent. If the queue is unreachable, stop and report it.

- One active build command, two compile jobs inside it: Cargo `-j 2`, Zig `-j2`.
- Cap test concurrency separately — Cargo's `-j` leaves test threads alone; pass `-- --test-threads=2` to Rust's standard harness when needed.
- Job limits are not a RAM ceiling. Lower concurrency under memory pressure; raise it only after measuring whole-build peak memory, linkers, test processes, and background tools included.
- Set the execution timeout to the expected build duration. After a tool timeout, poll the task already queued or running; resubmitting duplicates it.
- While editing, run the smallest relevant check. Before completion, run the focused tests and the project's required integrated validation — a passing check proves nothing about tests.

With [Agent Task Queue](https://github.com/block/agent-task-queue) configured:

```bash
tq cargo check -p <crate> --tests -j 2
tq zig build test -j2 --summary all
```

## Workers and snapshots

Edit in private worktrees; build in **one or two persistent workers**, each with a stable source path and its own build directories. N worktrees then share two warm build trees instead of growing N cold ones.

- Submit an immutable **snapshot** of every build input: uncommitted and untracked files and deletions included. Leave the worker's source untouched until the build and result collection finish.
- Install snapshots so every changed file gets a fresh mtime; new content under an old timestamp hides the change from rebuild detection.
- Record each run's snapshot identity, toolchain, command and options, exit status, and logs. A result validates only its own snapshot: reuse it for matching inputs, and run again after any later edit.
- Share dependency downloads and the compiler caches that support sharing. Each worker or worktree owns its live Cargo target / Zig build directory ([shared-directory hazard](https://github.com/rust-lang/cargo/issues/17312)).
- No workers available: keep private worktree targets and queue their builds. That bounds RAM, not disk duplication.

## Collection

Keep two budgets: shared caches (dependency downloads, sccache, Zig global cache) and worker outputs. The coordinator checks both after each build and on an interval while idle.

- **Stop-the-world**: queue collection through the build lock, and start only once every cache user has finished. File-age deletion outside the queue races live builds.
- **Roots** always survive: source changes, modified `zig-pkg` contents, and active workers' warm state. Garbage is old worker build directories that no active worker uses.
- Native GC first — each toolchain file says what it covers. Script cleanup only for what it leaves.
- Seed a new cache from an idle baseline with copy-on-write snapshots or reflinks (`cp --reflink`); hard links would share mutable build files between caches.
- Collection replaces routine full cleans, which discard the warm state the budget protects. Watch mode runs on workers only.

Done when every budget is back under its limit and every root is intact.
