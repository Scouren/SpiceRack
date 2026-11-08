---
name: rust
description: Zero-residue Rust, edition 2024 on Rust 1.95, where an abstraction enters hot code only after a codegen probe shows it costs nothing. Use when writing or reviewing a `.rs` file or a Rust `Cargo.toml`; when choosing between an abstraction and the plain loop; or when a Rust loop needs explicit SIMD.
---

# Rust, zero residue

Two words carry this skill.

- **Residue** is whatever an abstraction leaves in the machine code beyond the hand-written loop: a call, an indirect call, a panic path, a lost vector loop, extra instructions. A newtype, a trait, an iterator chain or a SIMD wrapper enters hot code only when a probe shows zero or negative residue ([`zero-cost.md`](zero-cost.md)). "Rust abstractions are zero-cost" is a claim to test, not a premise.
- **Boundary** is where outside data (bytes, files, users, plugins, FFI) becomes typed data. Everything is checked there once, with parsing, refined types and `unsafe` confined to it. Code past the boundary trusts its types and checks nothing again.

## Baseline

Every crate is `edition = "2024"`, `rust-version = "1.95"`, workspace `resolver = "3"`, and inherits the workspace lints:

```toml
[workspace.lints.rust]
unsafe_code = "forbid"
missing_docs = "warn"

[workspace.lints.clippy]
dbg_macro = "warn"
todo = "warn"
```

The gate for every change is `cargo clippy -p <crate> -- -D warnings`, `cargo fmt`, `cargo test -p <crate>`, plus the residue script when the crate has probes. These are Schemify's rules (`docs/dev/contributing.md`). The skill assumes them wherever a project does not say otherwise.

Use the language and std at 1.95. Each item below compiled and ran under `rustc --edition 2024` on 1.95.0:

| Feature | Reach for it when |
|---|---|
| `if let Some(x) = a && let Some(y) = b && x < y` (let chains, edition 2024) | a nested `if let` pyramid or a `match` with a guard |
| `let Some(h) = buf.split_first_chunk::<4>() else { return None };` | early exit on a failed pattern |
| `[const { Vec::new() }; N]`, `const { assert!(..) }` (inline const) | array repeat of a non-`Copy` value; a compile-time check inside a function |
| `std::hint::select_unpredictable(c, a, b)` | a data-dependent select that should not become a branch (the `branchless` skill) |
| `unsafe { std::hint::assert_unchecked(d != 0) }` | FFI crates only, after a probe shows a check that a safe `assert!` cannot remove |
| `std::hint::cold_path()` | the rare arm of a branch. **Stable on 1.95** (it was unstable on 1.91). `hint::likely`/`unlikely` are still unstable (E0658) |
| `impl Iterator<Item = u32> + use<'a>` (precise capturing) | a returned `impl Trait` that must not capture every lifetime in scope |
| `static T: LazyLock<..> = LazyLock::new(..)` | a lazily built global. Prefer passing the value down; this is for true process globals |
| `NonZero<u32>` (generic form) | a value that is never zero, and a free niche ([`types.md`](types.md)) |
| `split_first_chunk::<N>()`, `first_chunk`, `std::array::from_fn(|i| ..)` | fixed-size headers and arrays without index arithmetic or panics |
| `div_ceil`, `isqrt`, `unbounded_shl`/`unbounded_shr` | chunk counts, integer roots, shifts by ≥ the bit width (which return 0 instead of panicking) |
| `#[unsafe(no_mangle)]`, `unsafe extern "C" { safe fn .. }` | probes and FFI. Edition 2024 requires the `unsafe(...)` spelling, and the `unsafe_code` lint rejects it in forbid crates |

`std::simd` is still unstable on 1.95 (`portable_simd`). Explicit SIMD goes through `fearless_simd` ([`simd-first.md`](simd-first.md)).

## Writing a `.rs` file

1. **Types at the boundary.** Give units and IDs `#[repr(transparent)]` newtypes, and cross-table references typed index handles. Use `NonZero` where a handle is optional. Model variation as closed enums with exhaustive matches, and leave the `_` arm off any boundary enum. Use typestate only at an API boundary, and parse untrusted values once into refined types. Rules, measured layouts and compiling examples: [`types.md`](types.md). Done when every value from outside enters through one `parse`/`TryFrom`, every type stored in a large table has a `const` size assertion, and no `match` on a boundary enum has a `_` arm.
2. **Memory, errors, `unsafe`.** Store graphs as a `Vec` plus indices, not `Rc<RefCell>`. Size containers up front (`with_capacity`, `Box<[T]>`, `Cow`). Library errors are `thiserror` enums. `unwrap`/`expect` appear only for stated invariants, and `unsafe` only in the FFI crate, each block under a `// SAFETY:` comment. Rules and examples: [`memory.md`](memory.md). Done when no `Rc<RefCell<_>>` holds a table, every `expect` message names its invariant, every library error is a typed enum, and `unsafe` appears only in a crate that sets `clippy::undocumented_unsafe_blocks = "deny"`.
3. **Hot loops.** Triage with the `simd-loops` skill first: element type, length, and whether the dependency is none, accumulator or chain. When the autovectorized loop is not enough, or must be guaranteed, write a `#[simd]` kernel with `fearless_simd` 1.1. Detect the `Level` once at init, keep the scalar tail as the W == 1 oracle, and write the differential test. Read [`simd-first.md`](simd-first.md) before writing any `#[simd]` function. Done when every hot loop has a triage verdict, and every `#[simd]` function has a scalar oracle and a test that sweeps lengths 0 through past 3× the lane count on every level.
4. **Admit abstractions by residue.** Every abstraction on a hot path (a newtype, a generic, an iterator adapter chain, a `dispatch!` call site, a handle type) gets a `gen_`/`hand_` pair in the probe crate and a verdict from `scripts/asm_probe.py`. Read [`zero-cost.md`](zero-cost.md) for the protocol, the CI step and the 1.95 lab table before arguing about cost from memory. Done when every hot-path abstraction has a pair whose verdict is FOLDED, IDENTICAL, EQUIVALENT or NEGATIVE, or it has been replaced by the hand loop.
5. **Gate.** Done when `cargo clippy -p <crate> -- -D warnings`, `cargo fmt --check`, `cargo test -p <crate>` and `scripts/residue.sh` (when the crate has probes) all pass, and `cargo doc -p <crate>` prints no warnings.

Program shape (tables, transforms, spine) is `codebase-clean-code`'s. Library boundaries are `api-design`'s. Every `pub` item's `///` follows `doc-comments`.

## Reviewing a `.rs` change

Check each item against the diff and give a verdict with `file:line`:

- [ ] **boundary**: untrusted input is parsed once into a refined type; no `&str`/`u32` with rules travels into the core
- [ ] **types**: units and IDs are `#[repr(transparent)]` newtypes; hot-table types have a size assertion
- [ ] **matches**: no `_` arm on a boundary enum; `_` appears only for foreign `#[non_exhaustive]` enums, with a comment
- [ ] **dispatch**: no `dyn`/`Box<dyn>`/fn-pointer per element in a hot path (`dyn` probes as RESIDUE: an indirect call per element and no vector loop)
- [ ] **memory**: no `Rc<RefCell>` graphs; hot `Vec`s sized; fixed tables are `Box<[T]>`
- [ ] **errors**: library errors are `thiserror` enums; every `expect` names its invariant; no `unwrap` outside tests
- [ ] **unsafe**: none outside the FFI crate; every block has `// SAFETY:`
- [ ] **SIMD**: kernels use `fearless_simd`; `Level::new()` is not called per call; each kernel has its oracle test
- [ ] **residue**: each new hot-path abstraction has a probe pair; performance claims cite a benchmark, not a probe

Done when every box has a verdict, and every RESIDUE or missing probe is listed as a finding with the fix (rewrite, hand loop, or a new pair).

## Rulings

- **Iterators versus indexing is decided by the probe, not by style.** On 1.95, indexed and `zip` loops both vectorized. `zip` dropped the panic path but emitted more code in a store loop. Neither vectorized a float sum. Write what reads best and probe the hot ones.
- **`dyn` lives only at a plugin or FFI boundary.** Inside the core, variation is a closed enum or a generic (`codebase-clean-code` rule 4, `api-design/rust.md`).
- **`fearless_simd` over `std::simd`, `wide`, `pulp` or raw `core::arch`** for explicit SIMD. It is stable, safe, and dependency-free. `simd-loops/rust.md` describes the alternatives, and this ruling supersedes its choice among them.

## Already covered elsewhere

Point at these rather than restating them. `data-oriented-design/rust.md`: SoA crates, `slotmap`, `phf`, `bitvec`, `Option` versus `Result`, `?`, `anyhow` in binaries, `bumpalo`, Rayon and `Send`/`Sync`. `api-design/rust.md`: enums over `dyn`, typestate for init order, slices at the boundary, `*_into` caller-owned output. `branchless/rust.md`: the `select_unpredictable` and mask idioms. `simd-loops/rust.md`: the five-part shape and target-feature gotchas.

## The snippet bar

Every Rust snippet this skill ships, and every one you put in a doc, skill, review comment or PR description, compiles as written. A std-only snippet compiles with `rustc --edition 2024 --crate-type=lib` (with `--test` and a run when it has tests). A snippet that uses a crate (`thiserror`, `fearless_simd`) compiles and tests in a cargo project. All snippets in this skill met that bar on Rust 1.95.0 and are clippy-clean under `-D warnings`, as is `scripts/probe_template.rs`. Done when every snippet you added has been compiled, not read.

## Sources

- Schemify `Cargo.toml` (workspace lints, profiles) and `docs/dev/contributing.md` (hard rules 2, 5, 6, 7).
- The Rust Performance Book, nnethercote.github.io/perf-book.
- `fearless_simd` 1.1.0 README and `MANUAL_INLINING.md`, github.com/linebender/fearless_simd.
- Rule ideas mined from the `rust-skills` collection (`rules/`), each re-verified here rather than copied.
