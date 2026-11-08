# The shape in Rust

## The four steps

| Step | Rust |
|---|---|
| condition | ordinary comparison → `bool` |
| widen to 0/1 | `c as usize` / `c as u8` |
| widen to a mask | `(c as u64).wrapping_neg()` → `0` or `!0` |
| combine, arithmetic | `s += (p as u32) * x;` |
| combine, mask | `x & mask`, or the blend `b ^ ((a ^ b) & mask)` |
| combine, native | `a.min(b)`, `a.max(b)`, `x.clamp(lo, hi)`, `i32::abs`, `unsigned_abs`, `signum` |
| combine, select | `std::hint::select_unpredictable(c, a, b)` — **stable since 1.88** |
| saturate / wrap | `saturating_add`, `wrapping_mul`, … |
| bit counting | `count_ones`, `trailing_zeros`, `leading_zeros` |
| defeat the optimizer | `std::hint::black_box` — stable since 1.66 |

`hint::select_unpredictable` is the one to reach for; `slice::binary_search_by` uses it internally. Note there is no `bool::select_unpredictable` and no `<[T]>::select_unpredictable` — both names circulated during development and neither shipped. `hint::cold_path()` is stable since 1.95; `hint::likely`/`unlikely` are still unstable.

## The idioms

```rust
use std::hint::select_unpredictable;

// conditional count — lowers to adc on x86-64, cinc on AArch64.
// Prefer this over `ptr = ptr.add(cond as usize)`, which codegens worse.
n += (x > threshold) as usize;

// always store, conditionally advance — the compaction idiom.
// out must be sized to input.len(): you always write.
let mut out = vec![0.0; input.len()];
let mut n = 0;
for &x in input {
    out[n] = x;
    n += (x > threshold) as usize;
}
out.truncate(n);

// mask smear, then blend
let mask = (c as u64).wrapping_neg();
let r = b ^ ((a ^ b) & mask);

// fixed trip count, no early exit — essentially what std's binary search does
let (mut base, mut len) = (0usize, arr.len());
while len > 1 {
    let half = len / 2;
    base = select_unpredictable(arr[base + half - 1] < q, base + half, base);
    len -= half;
}
```

## Gotchas

- **Panics are branches.** Indexing, `unwrap`, slicing, and debug-mode overflow checks each insert a bounds test the mask form was supposed to remove. Iterate slices rather than indices so LLVM elides the check, or use `wrapping_*` / `get_unchecked`.
- **`Option` and `Result` reintroduce control flow.** `checked_*` returns an `Option`, which is a branch unless consumed branchlessly. `unwrap_or`/`map_or` on `Copy` types often become a cmov — verify rather than assume.
- **The lowering is not guaranteed.** The `select_unpredictable` docs say so explicitly, warn it "might decrease performance if `condition` is well-predictable", and rule it out for cryptographic constant-time code. Rust's binary search stopped being branchless at 1.25 purely from an LLVM change, so re-check the assembly after a toolchain bump.
- **`std::simd` is nightly.** `#![feature(portable_simd)]`; `Simd`, `Mask`, `mask.select(a, b)`, `simd_gt`. On stable, the vector form of the shape means `wide`/`pulp`, explicit `core::arch` intrinsics, or leaning on the autovectorizer once the branch is gone.
- **`cargo asm --rust --bin mybin 'mycrate::hot_fn'`** (from `cargo-show-asm`) is the fastest way to check what actually got emitted.
