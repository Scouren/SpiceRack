//! Residue probe: does each abstraction compile to the loop it claims to equal?
//!
//! Protocol (zero-cost.md has the full version):
//!
//! 1. Copy this file to `probes/src/lib.rs` in a crate of its own. `#[unsafe(no_mangle)]` trips
//!    the `unsafe_code` lint, so the probe crate does not inherit the workspace
//!    `forbid(unsafe_code)` (leave out `[lints] workspace = true`). Ship nothing from it.
//! 2. One pair per abstraction: `gen_<name>` calls the abstraction the way production code
//!    does; `hand_<name>` is the loop over primitives you would write without it. The names
//!    must match after the prefix, because CI derives the pair list from them.
//! 3. Both sides are `#[unsafe(no_mangle)] pub extern "C"`: the symbol keeps its name and
//!    both sides share one ABI. Slices cross `extern "C"` as (ptr, len) register pairs, which
//!    is why `improper_ctypes_definitions` is allowed below. `extern "C"` also adds a
//!    `panic_cannot_unwind` landing pad to any body that can panic; it appears on both sides.
//! 4. Generic and `#[inline]` items from the crate under test are instantiated here, so their
//!    bodies land in this listing. A non-generic item is inlined across crates only when rustc
//!    judges it small; otherwise it stays a `call`, which is real residue without LTO.
//! 5. Build and judge from the workspace root. asm_probe exits 1 on RESIDUE or a missing symbol.
//!    Verdicts are static: benchmark before claiming a speedup.
//!
//! ```sh
//! cargo rustc -p probes --release --lib -- --emit asm -C codegen-units=1 \
//!   -C llvm-args=-x86-asm-syntax=intel -C target-cpu=x86-64-v3
//! asm="$(ls -t target/release/deps/probes-*.s | head -1)"
//! grep -oP 'fn gen_\K\w+' probes/src/lib.rs | sed 's/.*/gen_&=hand_&/' \
//!   | xargs python3 asm_probe.py "$asm"
//! ```
#![allow(improper_ctypes_definitions)]

// Replace these with `use your_crate::{...}`; they are inline so the template builds alone.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Volts(pub f64);

pub trait Gain {
    fn apply(&self, x: f64) -> f64;
}

pub struct Double;

impl Gain for Double {
    fn apply(&self, x: f64) -> f64 {
        x * 2.0
    }
}

fn amplify<G: Gain>(g: &G, xs: &mut [f64]) {
    for x in xs {
        *x = g.apply(*x);
    }
}

// Newtype: expected FOLDED (LLVM merges it with hand_volts).
#[unsafe(no_mangle)]
pub extern "C" fn gen_volts(xs: &mut [Volts]) {
    for v in xs {
        v.0 *= 2.0;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn hand_volts(xs: &mut [f64]) {
    for x in xs {
        *x *= 2.0;
    }
}

// Generic + trait: expected IDENTICAL (monomorphized, vectorized, no calls).
#[unsafe(no_mangle)]
pub extern "C" fn gen_gain(xs: &mut [f64]) {
    amplify(&Double, xs);
}

#[unsafe(no_mangle)]
pub extern "C" fn hand_gain(xs: &mut [f64]) {
    for x in xs {
        *x *= 2.0;
    }
}
