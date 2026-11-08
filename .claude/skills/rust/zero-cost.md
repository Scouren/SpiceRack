# Zero cost, proven by residue

Reference for [`SKILL.md`](SKILL.md) step 4. **Residue** is whatever an abstraction leaves in the machine code beyond what the hand-written loop has: an extra call, an indirect call, a panic path, a lost vector loop, extra instructions. An abstraction enters hot code only when a codegen probe shows zero or negative residue against the loop it replaces. "Zero-cost" is a verdict from the probe, never a property of a language feature. The word means the same in the `zig` skill's `zero-cost.md`.

## The probe protocol

1. **Write the pair.** Copy [`scripts/probe_template.rs`](scripts/probe_template.rs) to `probes/src/lib.rs` in its own workspace member (`publish = false`, no `[lints] workspace = true`, because `#[unsafe(no_mangle)]` trips `forbid(unsafe_code)`). For each abstraction, add `gen_<name>`, which calls it exactly as production code does, and `hand_<name>`, the loop over primitives it claims to equal. Both are `#[unsafe(no_mangle)] pub extern "C"`. Done when every abstraction you are admitting has a pair and the names match after the prefix.
2. **Build to assembly** with one codegen unit, the release profile, and the target CPU you ship:

   ```sh
   cargo rustc -p probes --release --lib -- --emit asm -C codegen-units=1 \
     -C llvm-args=-x86-asm-syntax=intel -C target-cpu=x86-64-v3
   ```

   `-C codegen-units=1` can go in `[profile.release]` instead. The listing lands at `target/release/deps/probes-<hash>.s`. Take the newest one, since old hashes linger. Done when the `.s` file exists and `grep -c gen_ file.s` is non-zero.
3. **Judge** with [`scripts/asm_probe.py`](scripts/asm_probe.py): `python3 asm_probe.py file.s gen_a=hand_a gen_b=hand_b`. Verdicts, best first: FOLDED (one body for both symbols), IDENTICAL, EQUIVALENT (same mnemonics, different registers), NEGATIVE (gen does strictly less static work, or is vectorized where hand is not), RESIDUE. On RESIDUE it prints both bodies side by side with `>` on differing lines. Done when every pair is FOLDED, IDENTICAL, EQUIVALENT or NEGATIVE, or the abstraction is rewritten or removed.
4. **Benchmark before claiming speed.** The verdict is static: it counts instructions in the listing, not instructions executed. "No residue" means the abstraction is free. It does not mean the loop is fast. Done when any performance claim in the PR cites a benchmark, not the probe.

Reading traps:

- **`extern "C"` adds a `panic_cannot_unwind` landing pad** to any body that can panic or calls something opaque. It shows on both sides of a pair and cancels out. Read it as one panic path, not a bug.
- **A `jmp`/`call` to an out-of-line helper hides the helper's body.** Probe the helper on its own, or let it inline. In particular, `fearless_simd`'s `dispatch!` outlines each non-baseline level into its own `vectorize_*` symbol, so the dispatcher's listing shows calls, not the kernel. Probe the kernel through a fixed token instead (`gen_k(t: fearless_simd::Avx2, ...)`, see [`simd-first.md`](simd-first.md)).
- **Cross-crate calls.** Generic and `#[inline]` items monomorphize into the probe crate, so their bodies appear. A non-generic function from another crate was inlined when it was a small leaf. A non-leaf one stayed a `call` (verified on 1.95). Without LTO, that call is real residue in production too.

## Running it in CI

The probe crate holds only admitted pairs, so any RESIDUE is a regression. `asm_probe.py` exits 1 on RESIDUE or a missing symbol, and the pair list comes from the function names:

```sh
#!/usr/bin/env bash
# scripts/residue.sh: fail the build when an admitted abstraction regresses.
set -euo pipefail
cargo rustc -p probes --release --lib -- --emit asm -C codegen-units=1 \
  -C llvm-args=-x86-asm-syntax=intel -C target-cpu=x86-64-v3
asm="$(ls -t target/release/deps/probes-*.s | head -1)"
grep -oP 'fn gen_\K\w+' probes/src/lib.rs | sed 's/.*/gen_&=hand_&/' \
  | xargs python3 scripts/asm_probe.py "$asm"
```

As a GitHub Actions step: `- run: scripts/residue.sh`. Pin the toolchain (`rust-toolchain.toml` or `rust-version`), because a compiler upgrade is the usual cause of a new RESIDUE. When the upgrade causes it, the probe has done its job: decide whether to rewrite the abstraction or accept the residue and delete the pair, and say which in the PR. Run `python3 scripts/asm_probe.py --self-test` once in the same job, so a broken probe cannot pass silently. This script, copied verbatim, was run against a two-crate lab workspace on Rust 1.95. It exited 0 with the template's pairs, and 123 (`xargs` passing on the probe's 1) after a `&dyn` pair was added.

## Lab evidence

Rust 1.95.0, `-C opt-level=3 -C codegen-units=1`, `asm_probe.py` verdicts, counts at `-C target-cpu=x86-64-v3`. The newtype, generic, `dyn`, sum and indexing rows were also run at baseline x86-64 (SSE2) and gave the same verdicts. The "1.91" column is the user's earlier lab.

| Pair (gen vs hand) | 1.91 claim | 1.95 verdict | What the listing shows |
|---|---|---|---|
| `#[repr(transparent)] Volts(f64)` scale loop vs raw `f64` loop | FOLDED | **FOLDED** | one body, 4×ymm unrolled, 0 calls |
| generic `run<O: Op>` with a concrete `Op` vs hand loop | vectorized, no calls | **IDENTICAL** | same 59 instructions, 15 vector, 0 calls |
| `&dyn Op`, `o.f(x)` per element vs hand loop | indirect call per element, scalar | **RESIDUE** | `call r12` in the loop, 0 vector instructions |
| `xs.iter().sum::<f32>()` vs indexed `s += a[i]` | scalar | **RESIDUE** (both scalar) | 36 vs 34 instructions, 0 vector on both. Strict FP order blocks vectorization. `Sum` starts from `-0.0`, so an empty slice sums to `-0.0` |
| `xs.iter().map(\|x\| x * x).sum::<f64>()` vs indexed | not tested | **RESIDUE** (both scalar) | 54 vs 52 instructions, 0 vector on both |
| `fearless_simd` `sum` kernel (Avx2 token) vs `iter().sum::<f32>()` | vectorized | **NEGATIVE** | `vaddps ymm` loop vs scalar `vaddss` |
| read-only `zip` vs indexed `a[i] * b[i]` (u32) | both vectorized | **NEGATIVE** | both vectorized, 28 vector instructions each. Indexed keeps 2 calls and 3 panic paths, LLVM having versioned the check out of the loop |
| store `zip` vs indexed `o[i] = a[i] + 1` | both vectorized; zip only deletes the panic path | **RESIDUE** | both have the same 32-wide ymm loop. Zip has no panic path, but 98 vs 54 instructions (an extra 4-wide loop and an 8× unrolled tail) |
| indexed with `let o = &mut o[..n]` up front vs plain indexed | slicing added checks | **RESIDUE** | adds a `slice_index_fail` path, 103 vs 54 instructions, same vector loop |
| `dispatch!(Level::new(), ...)` per call vs `dispatch!(level, ...)` with a stored level | 4 calls of overhead | **RESIDUE** | +2 calls (`Once::call` for the cached detection), +1 panic path, +28 instructions |
| `NodeId(u32)` handle index vs raw `u32` index | not tested | **EQUIVALENT** | see `types.md` |
| `NetId(NonZero<u32>)` index (stores i + 1) vs raw `u32` | not tested | **RESIDUE** | one extra instruction (the `- 1`) |
| `Option<NetId>` lookup vs `u32` with 0 as the sentinel | not tested | **EQUIVALENT** | |

**Lesson.** "Iterators are negative-cost" is folklore here. In this lab an iterator was sometimes better (no panic path), sometimes worse (more code for the same vector loop), and never vectorized a float reduction the indexed loop did not. Both shapes vectorized whenever the operation allowed it, because LLVM hoists or versions bounds checks out of simple loops. Pick the shape that reads best, then probe it. The probe decides; a style rule does not. Store loops, where code size matters, are the case where indexing won on 1.95.

These are static listings on one machine's target. None of them is a speed measurement.

## Build settings that move residue

From the Rust Performance Book, worth setting for the release profile of a hot binary and re-probing after:

- `codegen-units = 1`: more cross-function optimization, slower builds. The probe protocol assumes it.
- `lto = "fat"` (or `"thin"`): cross-crate inlining of non-generic functions, which removes the cross-crate `call` residue above. Benchmark it, since the book says the gain is "not always" there.
- `panic = "abort"`: drops unwinding tables and landing pads, when nothing uses `catch_unwind`.
- `-C target-cpu`: the baseline you ship decides which vector width the autovectorizer may use. Set it in `.cargo/config.toml`, and use runtime dispatch (`simd-first.md`) for anything above the baseline.

Per-crate `opt-level = "z"` for cold dependencies, as in Schemify's root `Cargo.toml`, trades their speed for binary size. Hot crates stay at 3.

## Sources

- The Rust Performance Book (Nethercote): build configuration, bounds checks, inlining. nnethercote.github.io/perf-book
- Schemify `docs/dev/contributing.md` hard rule 2 (abstractions only at zero or negative overhead) and its root `Cargo.toml`.
- Lab files: `scratchpad/lab/rust/{probes.rs,handles.rs,energy.rs,fs/,ws/}` (session scratchpad, not kept).
