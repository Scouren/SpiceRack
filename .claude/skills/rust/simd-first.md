# SIMD first, with `fearless_simd`

Reference for [`SKILL.md`](SKILL.md) step 3. Explicit SIMD in this codebase is written with **`fearless_simd` 1.1** and its `#[simd]` attribute: safe code, one kernel generic over the instruction set, runtime dispatch on x86, and a scalar fallback everywhere. Verified on crates.io for this skill: `fearless_simd` 1.1.0, `rust-version = 1.89`, zero dependencies (`libm` is optional, for `no_std`). The attribute lives in the separately versioned `fearless_simd_macros` 0.1.0, which pulls `syn`/`quote`/`proc-macro2` at build time only.

```toml
[dependencies]
fearless_simd = "1.1"
fearless_simd_macros = "0.1"

[dev-dependencies]
# Lets tests run the kernel on the scalar fallback level too.
fearless_simd = { version = "1.1", features = ["force_support_fallback"] }
```

This file covers the Rust mechanics. The triage (is this loop worth it, is the dependency none, accumulator or chain) and the five-part shape are the `simd-loops` skill's job; run its triage first. The lane-axis doctrine (the W == 1 oracle, differential tests) is the same as in the `zig` skill's `simd-first.md`. `std::simd` is still unstable on 1.95 (`portable_simd`, error E0658), which is one reason this codebase uses `fearless_simd`, not `std::simd`.

## The kernel

```rust
use fearless_simd::{Level, Simd, dispatch, prelude::*};
use fearless_simd_macros::simd;

/// Scalar tail and W == 1 oracle: the kernel at one lane.
pub fn sum_scalar(xs: &[f32]) -> f32 {
    xs.iter().sum()
}

#[simd]
pub fn sum<S: Simd>(simd: S, xs: &[f32]) -> f32 {
    let mut acc = S::f32s::splat(simd, 0.0);
    let mut chunks = xs.chunks_exact(S::f32s::LEN);
    for c in &mut chunks {
        acc += S::f32s::from_slice(simd, c);
    }
    acc.reduce_sum() + sum_scalar(chunks.remainder())
}

/// `out[i] = if xs[i] > t { xs[i] * a + b } else { 0 }`.
pub fn ramp_scalar(xs: &[f32], out: &mut [f32], t: f32, a: f32, b: f32) {
    for (o, &x) in out.iter_mut().zip(xs) {
        *o = if x > t { x.mul_add(a, b) } else { 0.0 };
    }
}

#[simd]
pub fn ramp<S: Simd>(simd: S, xs: &[f32], out: &mut [f32], t: f32, a: f32, b: f32) {
    assert_eq!(xs.len(), out.len(), "ramp: xs and out differ in length");
    let (tv, zero) = (S::f32s::splat(simd, t), S::f32s::splat(simd, 0.0));
    let mut xc = xs.chunks_exact(S::f32s::LEN);
    let mut oc = out.chunks_exact_mut(S::f32s::LEN);
    for (x, o) in (&mut xc).zip(&mut oc) {
        let v = S::f32s::from_slice(simd, x);
        v.simd_gt(tv).select(v.mul_add(a, b), zero).store_slice(o);
    }
    ramp_scalar(xc.remainder(), oc.into_remainder(), t, a, b);
}

/// Appends every `x > t` to `out`, in order.
pub fn keep_above_scalar(xs: &[f32], t: f32, out: &mut Vec<f32>) {
    out.extend(xs.iter().copied().filter(|&x| x > t));
}

#[simd]
pub fn keep_above<S: Simd>(simd: S, xs: &[f32], t: f32, out: &mut Vec<f32>) {
    let tv = S::f32s::splat(simd, t);
    let mut chunks = xs.chunks_exact(S::f32s::LEN);
    for c in &mut chunks {
        let v = S::f32s::from_slice(simd, c);
        let m = v.simd_gt(tv);
        let k = m.to_bitmask().count_ones() as usize;
        out.extend_from_slice(&v.compress(m).as_slice()[..k]);
    }
    keep_above_scalar(chunks.remainder(), t, out);
}

/// Built once at startup; `Level` is `Copy`, so pass it down, not `Level::new()`.
pub struct Engine {
    level: Level,
}

impl Engine {
    pub fn detect() -> Self {
        Engine { level: Level::new() }
    }
    pub fn total(&self, xs: &[f32]) -> f32 {
        dispatch!(self.level, simd => sum(simd, xs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn levels() -> [Level; 3] {
        [Level::new(), Level::baseline(), Level::fallback()]
    }

    /// Lengths 0 through past 3x the widest lane count, four misalignments, NaN and inf.
    fn inputs() -> impl Iterator<Item = Vec<f32>> {
        const MAX_LANES: usize = 16; // f32 lanes at AVX-512
        (0..=3 * MAX_LANES + 1).flat_map(|n| {
            (0..4).flat_map(move |skew| {
                let base: Vec<f32> = (0..n + skew).map(|i| (i as f32 * 0.37).sin() * 8.0).collect();
                let mut special = base.clone();
                if let Some(x) = special.get_mut(skew) {
                    *x = f32::NAN;
                }
                if let Some(x) = special.last_mut() {
                    *x = f32::INFINITY;
                }
                [base[skew..].to_vec(), special[skew..].to_vec()]
            })
        })
    }

    /// Reductions reassociate and `mul_add` may or may not fuse: compare with a tolerance.
    fn close(a: f32, b: f32) -> bool {
        (a.is_nan() && b.is_nan()) || a == b || (a - b).abs() <= 1e-4 * a.abs().max(1.0)
    }

    #[test]
    fn sum_matches_oracle() {
        for level in levels() {
            for xs in inputs() {
                let (got, want) = (dispatch!(level, simd => sum(simd, &xs)), sum_scalar(&xs));
                assert!(close(got, want), "{level:?} n={} got {got} want {want}", xs.len());
            }
        }
    }

    #[test]
    fn ramp_matches_oracle() {
        for level in levels() {
            for xs in inputs() {
                let (mut got, mut want) = (vec![0.0; xs.len()], vec![0.0; xs.len()]);
                dispatch!(level, simd => ramp(simd, &xs, &mut got, 0.5, 2.0, 1.0));
                ramp_scalar(&xs, &mut want, 0.5, 2.0, 1.0);
                assert!(got.iter().zip(&want).all(|(&g, &w)| close(g, w)), "{level:?} n={}", xs.len());
            }
        }
    }

    #[test]
    fn keep_above_matches_oracle() {
        for level in levels() {
            for xs in inputs() {
                let (mut got, mut want) = (Vec::new(), Vec::new());
                dispatch!(level, simd => keep_above(simd, &xs, 0.5, &mut got));
                keep_above_scalar(&xs, 0.5, &mut want);
                assert_eq!(got, want, "{level:?} n={}", xs.len());
            }
        }
    }

    #[test]
    fn engine_uses_the_stored_level() {
        assert!(close(Engine::detect().total(&[1.0; 37]), 37.0));
    }
}
```

The API, checked against the 1.1.0 source:

| Part | `fearless_simd` |
|---|---|
| kernel | `#[simd] fn k<S: Simd>(simd: S, ...)`. The first typed parameter carries the token. A vector or mask argument works too |
| native-width types | `S::f32s`, `S::u8s`, `S::mask32s`, ...; lane count `S::f32s::LEN` (a `const`) |
| fixed-width types | `f32x4<S>`, `u8x32<S>`, ... |
| splat / load / store | `S::f32s::splat(simd, x)`, `S::f32s::from_slice(simd, chunk)` (panics unless `chunk.len() == LEN`), `v.store_slice(out_chunk)` |
| lane ops | operators, `mul_add(a, b)` = `self * a + b` (one or two roundings, by platform; `mul_add_precise` rounds once), `min`, `max`, `sqrt`, `abs` |
| compare | `simd_gt`, `simd_lt`, `simd_eq`, `simd_ge`, `simd_le`, `simd_ne` → a mask |
| blend | `mask.select(if_true, if_false)` |
| reduce | `reduce_sum`, `reduce_min`, `reduce_max`, `reduce_product`; `mask.to_bitmask()` → `u64` |
| compaction | `v.compress(mask)` packs the selected lanes to the front; `expand` is the inverse |
| call it | `dispatch!(level, simd => k(simd, xs))` |
| raw intrinsics | `fearless_simd::kernel!(fn f(avx2: Avx2, ...) { ... })` |

`dispatch!` runs its body in a closure, so `return` and `?` inside it apply to the closure. Call one function and put `?` on the `dispatch!` result. Helpers called from a kernel get the target features only if they are `#[simd]`, `#[inline(always)]`, or reach SIMD through `simd.vectorize(...)`. A plain non-inline helper compiles for the baseline. `MANUAL_INLINING.md` in the crate has the version without the macro.

## Detect the level once

Build `Level::new()` once at init and pass the `Level` (it is `Copy`) to every call site, as `Engine` does. Probe on 1.95 (`zero-cost.md` lab table): `dispatch!(Level::new(), ...)` per call, compared with `dispatch!(level, ...)` on a stored level, adds 2 calls (`Once::call` on the cached detection), a panic path, and 28 instructions to every call. The user's 1.91 lab measured 4 calls. The kernel body itself is the same.

## Raw intrinsics with `kernel!`

When the portable API lacks an instruction (`pshufb`, `movemask`, `pclmulqdq`), `kernel!` wraps a function so `core::arch` intrinsics are safe to call inside it. Gate it on the architecture, and keep the portable path as the fallback:

```rust
use fearless_simd::{Level, prelude::*, u8x32};
#[cfg(target_arch = "x86_64")]
use std::arch::x86_64::{__m256i, _mm256_cmpeq_epi8, _mm256_movemask_epi8, _mm256_set1_epi8};

pub fn count_byte_scalar(hay: &[u8], needle: u8) -> usize {
    hay.iter().filter(|&&b| b == needle).count()
}

#[cfg(target_arch = "x86_64")]
fearless_simd::kernel!(
    fn count_byte_avx2(avx2: Avx2, hay: &[u8], needle: u8) -> usize {
        let n = _mm256_set1_epi8(needle as i8);
        let mut chunks = hay.chunks_exact(32);
        let mut found = 0;
        for c in &mut chunks {
            let v: __m256i = u8x32::from_slice(avx2, c).into();
            found += _mm256_movemask_epi8(_mm256_cmpeq_epi8(v, n)).count_ones() as usize;
        }
        found + count_byte_scalar(chunks.remainder(), needle)
    }
);

pub fn count_byte(level: Level, hay: &[u8], needle: u8) -> usize {
    #[cfg(target_arch = "x86_64")]
    if let Some(avx2) = level.as_avx2() {
        return count_byte_avx2(avx2, hay, needle);
    }
    let _ = level;
    count_byte_scalar(hay, needle)
}

#[test]
fn matches_scalar() {
    let hay: Vec<u8> = (0..1000u32).map(|i| (i * 7 % 13) as u8).collect();
    for n in [0, 1, 31, 32, 33, 999] {
        assert_eq!(count_byte(Level::new(), &hay[..n], 3), count_byte_scalar(&hay[..n], 3));
    }
}
```

`kernel!` accepts one safe, non-generic function whose first parameter is a bare token name (`Neon`, `WasmSimd128`, `Sse2`, `Sse4_2`, `Avx2`, `Avx512`).

## Probing a kernel

`dispatch!` outlines every non-baseline level into its own `vectorize_*` function, so a probe of the dispatcher shows calls, not the kernel. Probe the kernel through a fixed token, gated on the architecture, because the token type does not exist on other targets (an ungated probe broke the wasm build in the lab):

```rust
#![allow(improper_ctypes_definitions)]
use fearless_simd::{Simd, prelude::*};
use fearless_simd_macros::simd;

#[simd]
pub fn sum<S: Simd>(simd: S, xs: &[f32]) -> f32 {
    let mut acc = S::f32s::splat(simd, 0.0);
    let mut chunks = xs.chunks_exact(S::f32s::LEN);
    for c in &mut chunks {
        acc += S::f32s::from_slice(simd, c);
    }
    acc.reduce_sum() + chunks.remainder().iter().sum::<f32>()
}

#[cfg(target_arch = "x86_64")]
#[unsafe(no_mangle)]
pub extern "C" fn gen_sum_avx2(t: fearless_simd::Avx2, xs: &[f32]) -> f32 {
    sum(t, xs)
}

#[unsafe(no_mangle)]
pub extern "C" fn hand_sum_avx2(xs: &[f32]) -> f32 {
    xs.iter().sum::<f32>()
}
```

Built at `-C target-cpu=x86-64-v3`, this pair was NEGATIVE on 1.95: a `vaddps ymm` loop against the scalar `vaddss` chain that strict float ordering forces on `iter().sum()`. The fearless loop above accumulates into one register, so its speed is bounded by add latency. Whether several accumulators pay off is a benchmark question, covered by the `simd-loops` skill.

## Binary size and targets

- **x86 multiversioning compiles each kernel once per level** (SSE2, SSE4.2, AVX2, AVX-512). Measured on 1.95 for the three kernels above in a small binary: `.text` was 318,763 bytes with every level dispatched, 316,555 with `--cfg disable_dispatch_sse4_2 --cfg disable_dispatch_avx512`, and 315,019 with all four `disable_dispatch_*` cfgs. That is 3.7 KB for three kernels. `codegen-units = 1` against 16 changed it by 0.3 KB, and fat LTO cut 25 KB, mostly from std. Mitigate in this order: `codegen-units = 1` or LTO, then the `disable_dispatch_*` cfgs in `RUSTFLAGS`. Those cfgs only remove automatic multiversioning: the token types and `kernel!` still work.
- **wasm has no runtime feature detection: build twice.** Verified on 1.95 (`wasm32-unknown-unknown`): the kernels above compiled with no `f32x4.add` without `-Ctarget-feature=+simd128` and with it, 7. Ship both modules and pick one in the loader (`wasm-feature-detect`). Cargo cannot set per-profile `RUSTFLAGS`, so the two builds are two invocations in a script.
- **`no_std`**: `Level::new()` needs the `std` feature except on wasm. Without std, take the `Level` from the caller.

## The lane-axis doctrine

Every kernel has a **W == 1 oracle**: the scalar function that is both its tail and its reference (`sum_scalar`, `ramp_scalar` above). Every kernel ships with a differential test against that oracle, which must:

- sweep lengths from 0 through past 3× the widest lane count (16 f32 lanes at AVX-512), so the empty case, every partial tail, and several full chunks are hit;
- run at several misalignments (start the slice 0–3 elements into a buffer);
- inject NaN and ±inf, and compare NaN as equal to NaN;
- run on every level: `Level::new()`, `Level::baseline()`, and `Level::fallback()` (needs the `force_support_fallback` dev-dependency feature);
- compare exactly for integer and selection kernels, and with a stated tolerance for reductions and `mul_add`. A float `Sum` starts at `-0.0`, so an oracle built on `iter().sum()` returns `-0.0` for an empty slice.

The test skeleton is the `tests` module in the kernel block above. It compiles and passes on 1.95 (3 levels × 400 inputs per kernel, run on an AVX2 machine). Done when every `#[simd]` function in the crate has a test of this form.

## Sources

- `fearless_simd` 1.1.0: README, `MANUAL_INLINING.md`, `fearless_simd_macros/README.md`, `src/lib.rs` (`Level`), `src/macros.rs` (`dispatch!`), `src/kernel_macros.rs` (`kernel!`), `src/generated/simd_trait.rs`. github.com/linebender/fearless_simd
- crates.io metadata via `cargo info fearless_simd` / `cargo info fearless_simd_macros` (2026-10-16).
