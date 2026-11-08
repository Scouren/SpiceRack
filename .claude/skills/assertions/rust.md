# Assertions in Rust

Everything here was compiled with rustc 1.95.0, edition 2024, on x86-64 (`-C target-cpu=x86-64-v3`) and read with `asm_probe.py`.

## The three macros

| Form | Debug build | Release build (`opt-level=3`, default profile) | A false condition |
|---|---|---|---|
| `assert!(c)` | checked | checked | panic, in every build |
| `debug_assert!(c)` | checked | **compiled out**, condition not evaluated | panic only where `debug_assertions` is on |
| `unsafe { std::hint::assert_unchecked(c) }` | checked: aborts with `unsafe precondition(s) violated: hint::assert_unchecked must never be called when the condition is false` | no check; the optimizer takes `c` as a fact | **undefined behavior** in release |

Observed: a `debug_assert!(n == 7)` that is false panics in a debug build (exit 101), is silent at `-C opt-level=3`, and panics again at `-C opt-level=3 -C debug-assertions=on`. A false `assert_unchecked` aborts in the debug build (exit 134, non-unwinding) and runs on silently at `-C opt-level=3`. The debug-build message itself says the check "is optional, and cannot be relied on for safety".

Which to write:

- `assert!` for a precondition of a public function and for anything the skill's step 7 bounds. It is the only one that holds in production, and `# Panics` documents it (the `doc-comments` skill).
- `debug_assert!` for an invariant check too costly for release: an O(N) re-verification inside an O(1) operation, the Rust counterpart of TigerBeetle's `if (constants.verify)`. Run the test suite with debug assertions on, so it executes.
- `assert_unchecked` only where a probe shows the remaining check matters and a `// SAFETY:` comment proves the condition. It is `Zig ReleaseFast assert` semantics: a promise, not a check. A crate under `#![forbid(unsafe_code)]` (Schemify, `contributing.md` rule 6) cannot use it at all.

`assert_eq!(a, b)` and `assert_ne!` print both values on failure. Prefer them over `assert!(a == b)` when the values are `Debug`; the message is the first thing read in a fuzz crash.

## Forms

```rust
const BLOCK_SIZE: usize = 4096;
const HEADER_SIZE: usize = 128;
const _: () = assert!(HEADER_SIZE < BLOCK_SIZE);
const _: () = assert!(BLOCK_SIZE % HEADER_SIZE == 0);
const _: () = assert!(BLOCK_SIZE.is_power_of_two());

#[repr(C)]
struct Header { checksum: u64, size: u32, kind: u8, pad: [u8; HEADER_SIZE - 13] }
const _: () = assert!(size_of::<Header>() == HEADER_SIZE);

struct Queue { items: [u32; Queue::CAPACITY], count: usize }
impl Queue {
    const CAPACITY: usize = 64;
    fn push(&mut self, item: u32) {
        assert!(self.count < Self::CAPACITY, "queue full");
        self.items[self.count] = item;
        self.count += 1;
        debug_assert!(self.count <= Self::CAPACITY);
    }
}

fn add_into(a: &[u32], b: &[u32], o: &mut [u32]) {
    assert!(b.len() >= a.len());
    assert!(o.len() >= a.len());
    for i in 0..a.len() { o[i] = a[i].wrapping_add(b[i]); }
}
```

A false `const _: () = assert!(...)` is a compile error: `error[E0080]: evaluation panicked: assertion failed: A <= B`. Verified. These blocks compile and pass `rustc --test` with a `#[should_panic]` test for the short-output case.

## Negative cost: the up-front length assert

Probe file (`#[unsafe(no_mangle)]` keeps the symbol names; `wrapping_add` keeps overflow semantics out of the comparison):

```rust
#[unsafe(no_mangle)] pub fn add_none(a: &[u32], b: &[u32], o: &mut [u32]) {
    for i in 0..a.len() { o[i] = a[i].wrapping_add(b[i]); }
}
// add_compound:  assert!(b.len() >= a.len() && o.len() >= a.len()); then the same loop
// add_split:     assert!(b.len() >= a.len()); assert!(o.len() >= a.len()); then the same loop
// add_zip:       for ((x, y), z) in a.iter().zip(b).zip(o.iter_mut()) { *z = x.wrapping_add(*y); }
// add_debug:     debug_assert! ×2; then the same loop
// add_unchecked: unsafe { assert_unchecked(..) } ×2; then the same loop
```

```sh
rustc --edition 2024 --crate-type=lib -C opt-level=3 -C codegen-units=1 -C target-cpu=x86-64-v3 \
  --emit asm -C llvm-args=-x86-asm-syntax=intel a.rs -o rs.s
asm_probe.py rs.s add_compound=add_none add_split=add_compound add_zip=add_compound \
  add_debug=add_none add_unchecked=add_compound --report
```

| Variant | Instructions | Vector | Panic sites | What the loop looks like |
|---|---|---|---|---|
| `add_none` | 67 | 12 | 2 (`panic_bounds_check`) | LLVM computes `min(len)` with `cmov`, vectorizes up to it (4 × ymm per iteration), then a scalar tail that checks both bounds per element |
| `add_compound` | 63 | 15 | 1 | same 4 × ymm main loop, plus a 4-wide xmm epilogue and an unchecked scalar tail; checks only at entry |
| `add_split` | 68 | 15 | 2 | identical loop to `add_compound`; one extra panic call site, one per assert |
| `add_zip` | 58 | 15 | 0 | same vector loop; **no panic**: `zip` stops at the shortest slice |
| `add_debug` | 67 | 12 | 2 | EQUIVALENT to `add_none`: compiled out at opt-level 3 |
| `add_unchecked` | 54 | 15 | 0 | the `add_compound` loop without the entry checks |

What this means:

- Rust gains less than Zig. Without any assert LLVM already splits the loop and vectorizes the safe prefix, so the assert removes the checked scalar tail and a panic site, not the main loop's work. Write it for the contract; the codegen is a smaller bonus here.
- The split form keeps the loop of the compound form exactly; its cost is one cold panic call per assert, which is also what makes the failing condition visible.
- `zip` is not a drop-in for the asserted loop. It truncates silently where the assert panics, so a short output slice becomes a short write. Pair it with `assert_eq!` on the lengths when they must match.
- A read-only indexed loop (`s = s.wrapping_add(a[i].wrapping_mul(b[i]))`) vectorizes the same way with or without `assert!(b.len() >= a.len())` (85 vs 86 instructions, 28 vector each).

For how to write such a probe and when a codegen result still needs a benchmark, follow the `rust` skill's `zero-cost.md`.
