# The shape in Rust

Rust has three SIMD paths, and most Rust SIMD code found in the wild looks frightening because it took the third one. Choose deliberately:

1. **Autovectorization** — write the chunk loop with `chunks_exact` and let LLVM vectorize the body. Zero dependencies, stable, and often enough. Try this first.
2. **`std::simd`** — the portable API, still nightly-only behind `#![feature(portable_simd)]` (tracking issue #86656). Reads closest to the shape; costs a pinned nightly toolchain.
3. **`core::arch` intrinsics** — vendor-named, per-instruction-set, needing `#[target_feature]` and runtime detection. Reach here only for a specific instruction with no portable equivalent (`pclmulqdq`, `pshufb`). Most intrinsics became safe to call inside a matching `#[target_feature]` function in Rust 1.87.

On stable and needing more than autovectorization: use `fearless_simd` — safe, zero dependencies, runtime dispatch built in. The `rust` skill's `simd-first.md` holds the kernel shape, dispatch, and the per-kernel differential test; this file's five parts map onto it one for one.

## The five parts (`std::simd`)

| Part | Rust |
|---|---|
| lane count | a plain `const N: usize`, a power of two ≤ 64 |
| vector type | `Simd<T, N>`, aliases `u8x32`, `f32x8`, … |
| splat | `Simd::<u32, N>::splat(value)` |
| load a chunk | `Simd::<u32, N>::from_slice(&s[i..])` — panics on a short slice |
| lane op | operators apply elementwise; `simd_gt`, `simd_eq`, `simd_lt` return a `Mask` |
| reduce to bool | `mask.all()`, `mask.any()` |
| reduce to scalar | `v.reduce_sum()`, `reduce_max()`, `reduce_min()`, `reduce_and()` |
| mask → integer | `mask.to_bitmask()` → `u64`; `Mask` implements `Not`, so `(!mask)` inverts |
| locate a lane | `(!mask).to_bitmask().trailing_zeros()`, `mask.to_bitmask().count_ones()` |
| blend | `mask.select(a, b)` |
| store | `v.copy_to_slice(&mut out[i..])` |

`use std::simd::prelude::*` pulls in `Simd`, `Mask`, and the comparison traits (`SimdPartialOrd` and friends live in `std::simd::cmp`).

No lane-count guard is needed, unlike Zig: pick `N`, and on a target with narrower registers the compiler splits the operation into several instructions rather than falling back to scalar. Portable SIMD compiles for every target.

## The chunk loop and the tail come from the standard library

`chunks_exact` is parts 2 and 5 of the shape, already written, on stable:

```rust
let mut it = data.chunks_exact(N);
for chunk in it.by_ref() { /* vector body */ }
for &x in it.remainder() { /* scalar tail */ }
```

On Rust 1.95 an index-based loop over integer lanes vectorizes too (LLVM hoists or versions the bounds checks), so the chunk form's value is a predictable shape, not a guaranteed win; a float reduction stays scalar in either form. Probe both with the `rust` skill's `zero-cost.md` before writing explicit SIMD.

## Reduce kind: locate the first element

```rust
#![feature(portable_simd)]
use std::simd::{prelude::*, Simd};

const N: usize = 8;

fn scan(cps: &[u32]) -> usize {
    let mut end = 0;
    let threshold = Simd::<u32, N>::splat(0xF);

    while end + N <= cps.len() {
        let values = Simd::<u32, N>::from_slice(&cps[end..]);
        let above = values.simd_gt(threshold);
        if above.all() {
            end += N;
            continue;
        }
        return end + (!above).to_bitmask().trailing_zeros() as usize;
    }

    while end < cps.len() && cps[end] > 0xF {
        end += 1;
    }
    end
}
```

## Reduce kind: accumulate

```rust
fn count(haystack: &[u8], needle: u8) -> usize {
    const N: usize = 32;
    let needles = Simd::<u8, N>::splat(needle);
    let mut it = haystack.chunks_exact(N);
    let mut found = 0;

    for chunk in it.by_ref() {
        let v = Simd::<u8, N>::from_slice(chunk);
        found += v.simd_eq(needles).to_bitmask().count_ones() as usize;
    }

    found + it.remainder().iter().filter(|&&b| b == needle).count()
}
```

Popcount per chunk is fine for counting. For a float sum, carry a `Simd<f32, N>` accumulator across the whole chunk loop and call `reduce_sum()` once after it — and note that floating-point addition is non-associative, so the vector sum will differ in the last bits from the scalar one. Loosen the differential test to an epsilon comparison for float reductions rather than exact equality.

## Gotchas

- **Nothing vectorizes without target features.** Build with `-C target-cpu=native` (or set a baseline in `.cargo/config.toml`, or annotate with `#[target_feature]`). Default `x86-64` gives only SSE2, so an AVX2-shaped loop silently lowers to four times as many instructions.
- **`from_slice` panics when the slice is shorter than `N`.** The `while i + N <= len` guard or `chunks_exact` is what keeps it from firing.
- **Non-associative float reductions** change results versus scalar; this is also why LLVM refuses to autovectorize float sums without `-ffast-math`-style permission, and one of the main reasons a scalar float loop that "should" vectorize doesn't.
- **`std::simd` is unstable.** Its API has churned; pin the nightly in `rust-toolchain.toml` and expect the occasional rename when bumping.
