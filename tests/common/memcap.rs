//! The law binaries' memory cap: a binary that includes this module (`#[path =
//! "common/memcap.rs"] mod memcap;`) runs under an address-space limit of [`CAP_BYTES`], set
//! once by the loader before `main` (an ELF `.init_array` constructor), so every test of the
//! binary is covered, a test added later included, with no call to forget.
//!
//! Why: a law binary feeds generated or mutated text to a reader or a pass; a runaway read (a
//! loop that never ends on a mutant, a table that grows per token) once grew `dialect_mutants`
//! to 22 GB and took the machine and twenty agents with it. Under the cap the allocation that
//! crosses it fails, and Rust aborts that binary with `memory allocation of N bytes failed`.
//! The limit is inherited by the simulators a test starts (ngspice, VACASK), which need far
//! less.
//!
//! Linux only (`RLIMIT_AS`, `.init_array`); elsewhere the module is empty.
//!
//! Cites: setrlimit(2) (`RLIMIT_AS` counts reserved address space, resident or not).
//!
//! Contents: `pub const CAP_BYTES: u64`

#![allow(dead_code)]

/// The cap, 6 GiB of address space: above what any law binary reserves (each runs in well under
/// 1 GiB resident, four test threads; a whole fast gate peaks at 2.5 GB), and under the build
/// queue's scope (two slots of 10 GB each), so a runaway binary dies here before its scope
/// kills the whole command. The simulators the pair test starts inherit it, and need far less.
pub const CAP_BYTES: u64 = 6 << 30;

/// The constructor the loader runs before `main`.
#[cfg(target_os = "linux")]
#[used]
#[unsafe(link_section = ".init_array")]
static CAP_AT_START: extern "C" fn() = cap_at_start;

// ---- private ----

/// Lowers the soft `RLIMIT_AS` to [`CAP_BYTES`], keeping the hard limit; aborts when the
/// kernel refuses (a binary meant to be capped must not run uncapped).
#[cfg(target_os = "linux")]
extern "C" fn cap_at_start() {
    let mut limit = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: `getrlimit` writes one `rlimit` through a valid, exclusive pointer.
    let got = unsafe { libc::getrlimit(libc::RLIMIT_AS, &mut limit) };
    limit.rlim_cur = CAP_BYTES.min(limit.rlim_max);
    // SAFETY: `setrlimit` reads one initialized `rlimit` through a valid pointer.
    let set = unsafe { libc::setrlimit(libc::RLIMIT_AS, &limit) };
    if got != 0 || set != 0 {
        std::process::abort();
    }
}
