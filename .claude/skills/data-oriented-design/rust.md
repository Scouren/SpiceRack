# Rust — realizing the principles

Companion to foundations.md. Each section maps a principle to Rust mechanics. A
recurring theme: several data structures that are _native_ in Zig (MultiArrayList,
StaticStringMap, packed/bit-level layouts, index arenas) are **not** in Rust's std and
are supplied by well-established crates. Those are called out explicitly. Sections 4–5
cover Rust's intended handling of `Option`/`Result` and errors.

---

## 1. Data-Oriented Design in Rust — and the crates that fill the gaps

Rust gives you full control of layout (`#[repr(...)]`, field ordering, fixed arrays),
monomorphization, and slices, so the _transformations_ are easy to write idiomatically.
What it lacks is std-level SoA and arena containers, so you pull them in.

### Struct-of-Arrays — `soa_derive` (or `soa-rs`, `parallel_vec`)

std has no `MultiArrayList`. The standard substitute is `soa_derive`:

```rust
use soa_derive::StructOfArray;

#[derive(StructOfArray)]
pub struct Particle { pub pos: [f32; 3], pub vel: [f32; 3], pub color: u32, pub alive: bool }
// generates ParticleVec { pos: Vec<[f32;3]>, vel: Vec<[f32;3]>, color: Vec<u32>, alive: Vec<bool> }
// plus ParticleSlice / ParticleRef helper types.

let mut ps = ParticleVec::new();
ps.push(Particle { pos: [0.0;3], vel: [1.0,0.0,0.0], color: 0, alive: true });
for (p, v) in ps.pos.iter_mut().zip(&ps.vel) { p[0]+=v[0]; p[1]+=v[1]; p[2]+=v[2]; }
```

**Caveat (real, and worth stating):** the generated `ParticleVec` cannot implement
`Deref`/`Index` to return a reference, because the slice/ref types are _not_ references.
So `ps[i]` and `&ps[i]`-style indexing don't work the way they do on `Vec<T>`; you use
the generated `ParticleRef`/`as_ref()` accessors. This is a genuine ergonomic cost Zig's
`MultiArrayList` doesn't have. For hand-rolled control, plain parallel `Vec`s are always
an option.

### Indices instead of pointers — `slotmap`, `generational-arena`, `thunderdome`

std has no arena keyed by stable indices. These crates give you `u32`/`u64`-style keys,
O(1) insert/remove, and protection against stale keys via generations:

```rust
use slotmap::{SlotMap, new_key_type};
new_key_type! { struct NodeKey; }            // a distinct, non-arithmetic key type

let mut nodes: SlotMap<NodeKey, Node> = SlotMap::with_key();
let root = nodes.insert(Node { /* ... */ }); // `root: NodeKey` replaces a pointer
```

This is the DOD "pointers become positions in an arena" move, with generational keys
preventing use-after-free of a recycled slot. `id-arena`/`typed-arena` cover the
append-only / no-deletion cases. For typed integer indices into your own `Vec`, the
`index_vec` crate gives newtype-checked indices.

### Compile-time perfect-hash maps — `phf`

The analogue of Zig's `StaticStringMap`: a map built entirely at compile time, no
runtime hashing of a fixed key set.

```rust
use phf::phf_map;
static KEYWORDS: phf::Map<&'static str, Token> = phf_map! {
    "fn" => Token::Fn, "const" => Token::Const, "return" => Token::Return,
};
```

### Bit-level packing — `bitvec` (and `bitflags`, `enumset`)

std has `Vec<bool>` (one byte per bool) but no bit-addressable storage. `bitvec` gives a
true bitset/bit-slice (the "booleans out of band, one bit each" technique); `bitflags`
gives packed flag enums; `enumset` gives a bitset keyed by an enum.

### Shrinking elements and avoiding tiny heap allocs — `smallvec`, `arrayvec`, `tinyvec`

Inline small-buffer storage that spills to the heap only when it grows. Keeps small
collections off the heap and elements cache-resident. Reorder fields / pick the smallest
integer types yourself; Rust will lay out `repr(Rust)` structs without you fighting it,
and `#[repr(C)]`/`packed` give exact control when an external layout matters.

### Encodings instead of polymorphism — enums and monomorphization

Prefer a closed `enum` + `match` over `Box<dyn Trait>` in hot loops. An enum is a tag
plus payload (DOD's "encoding"), branched at the top of the loop; `dyn` is a vtable
indirection per element that blocks inlining and vectorization. Benchmarks of this exact
swap (and of SoA vs AoS, and contiguous `Vec` vs linked list) routinely show 50–90%
differences. Reserve `dyn` for genuinely open-ended, cold sets. If you want enum
dispatch without writing the `match` boilerplate, the `enum_dispatch` crate generates it.

---

## 2. Negative-overhead abstraction in Rust

Rust is the poster child for the operational test in the theory doc: high-level code
that compiles to what you'd hand-write.

### Iterators monomorphize; measure before claiming they beat indexing

```rust
fn energy(xs: &[f64]) -> f64 { xs.iter().map(|x| x * x).sum() }
```

`map`/`sum` are inlined; no closure survives to runtime. That part is zero-cost. The
loop is still scalar on Rust 1.95: a float `sum` keeps strict left-to-right ordering, so
LLVM will not reassociate it into lanes. Indexed loops vectorize as readily as iterator
loops on 1.95 (LLVM hoists or versions the bounds checks). "Iterators are negative-cost"
is folklore until a probe says so: the `rust` skill's `zero-cost.md` has the measured
cases and the probe protocol, and `simd-first.md` has the vectorized float reduction.

### Type-state: invariants at compile time, zero bytes at runtime

```rust
use core::marker::PhantomData;
struct Open; struct Closed;                  // zero-sized markers
struct File<S> { fd: i32, _s: PhantomData<S> }
impl File<Closed> { fn open(p: &str) -> File<Open> { /* ... */ } }
impl File<Open>   { fn read(&self) -> Vec<u8> { /* ... */ }
                    fn close(self) -> File<Closed> { /* ... */ } }
// read() on a Closed file is a compile error; size_of::<File<Open>>() == size_of::<i32>().
```

### The honest caveat

A newtype wrapper can, in specific cases, defeat autovectorization the underlying type
would have gotten; `Box<dyn Trait>` and `Arc<T>` are real indirection/atomics, not free.
So on hot paths, verify with the `rust` skill's codegen probe and a `criterion` benchmark rather than
trusting "zero-cost" as a slogan.

---

## 3. API design in Rust (Muratori's rules, mostly enforced by the type system)

- **Caller owns memory and lifetime (low coupling, optional resource management).**
  Ownership and borrowing make this the default: take `&mut Vec<T>`/`&mut [T]` outputs
  or an allocator-like sink instead of allocating internally when the caller might want
  to reuse buffers. Offer a `*_into(&mut out)` primitive alongside an allocating
  convenience method.
- **Transparent data unless there's an invariant.** Plain `pub` fields for field-bags;
  privacy + methods only where a type maintains an invariant. Module
  privacy (`pub`, `pub(crate)`) is the real encapsulation unit.
- **Granularity / build convenience on primitives.** Expose `parse_into(buf, &mut ast)`
  and build `parse(buf) -> Result<Ast>` on top of it; never offer only the coarse call.
- **Errors are part of the signature** (next sections): a precise error type is a
  checked contract, the Rust analogue of Zig's error set.

---

## 4. `Option` vs `Result` — absence vs failure

Rust draws the same line Zig does. **`Option<T>` for expected absence**; **`Result<T,E>`
for genuine failure.** A lookup miss is `None`, not an `Err`.

```rust
fn find(hay: &[u8], needle: u8) -> Option<usize> {
    hay.iter().position(|&c| c == needle)        // ordinary "not found" => None
}

let i = find(s, b'/').unwrap_or(s.len);          // unwrap-or-default
if let Some(idx) = find(s, b'/') { use_it(idx) } // capture if present
while let Some(item) = it.next() { process(item) }
```

Reserve `.unwrap()`/`.expect()` for cases the surrounding logic has already guaranteed
(and prefer `.expect("why this can't fail")` so the panic message documents the
invariant). Don't model expected absence as an error.

---

## 5. Error handling — `Result`, `?`, and the library/application split

Failures are values: `Result<T, E>`. Propagate with `?`, which returns the error from
the current function (converting it via `From` along the way).

```rust
fn parse_u32(bytes: &[u8]) -> Result<u32, ParseError> {
    let mut acc: u32 = 0;
    for &c in bytes {
        if !c.is_ascii_digit() { return Err(ParseError::Unexpected(c)); }
        acc = acc.checked_mul(10).ok_or(ParseError::Overflow)?;   // Option -> Result -> ?
        acc += (c - b'0') as u32;
    }
    Ok(acc)
}
```

### Libraries: typed errors with `thiserror`

A library returns a _typed_ error enum so callers can discriminate. `thiserror` derives
`std::error::Error`, `Display`, and the `From` conversions that make `?` compose:

```rust
#[derive(thiserror::Error, Debug)]
pub enum ParseError {
    #[error("unexpected byte: {0:#x}")] Unexpected(u8),
    #[error("integer overflow")]        Overflow,
    #[error(transparent)]               Io(#[from] std::io::Error), // `?` on io::Error just works
}
```

### Applications: `anyhow` (or `eyre`) at the top

Application/binary code that just needs to fail with good context uses a boxed,
type-erased error and adds context as it propagates:

```rust
use anyhow::{Context, Result};
fn load(path: &str) -> Result<Config> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {path}"))?;
    let cfg = parse(&bytes).context("parsing config")?;   // ParseError flows in via `?`
    Ok(cfg)
}
```

### Conventions

- `Option` for expected absence, `Result` for failure. Convert between them with
  `ok_or`/`ok_or_else` and `.ok()` as needed.
- **Libraries return typed errors** (`thiserror`); **applications may type-erase**
  (`anyhow`/`eyre`). Don't put `anyhow` in a library's public API — it robs callers of
  the ability to match on the failure.
- Propagate with `?`; reserve `panic!`/`unwrap`/`expect` for truly-impossible cases and
  tests, and document the invariant in the `expect` message.
- Don't ignore a `Result`; the `#[must_use]` lint exists for this reason.

---

## 6. Memory and allocation in Rust

Rust's global allocator is implicit (you don't pass one), so the discipline shifts to
controlling _allocation patterns_ and reaching for arenas where they pay.

- **Reserve up front.** `Vec::with_capacity(n)` / `vec.reserve(n)` before a loop avoids
  repeated reallocation and copying as it grows. This is the highest-frequency win.
- **Clear and reuse, don't reallocate.** `vec.clear()` keeps the capacity; reuse one
  buffer across iterations instead of allocating a fresh `Vec` each time.
- **Arenas via crates** (std's allocator API is still unstable): `bumpalo` is a bump
  arena that frees everything at once; `typed-arena` holds one type and hands out
  `&mut` references with the arena's lifetime. Both collapse per-object frees into one.

```rust
use bumpalo::Bump;
let arena = Bump::new();                 // a phase-scoped region
let node = arena.alloc(Node { /* ... */ });   // freed all at once when `arena` drops
```

- **Caller-owned output** (Muratori's retention): take `&mut Vec<T>` / `&mut [T]` and
  write into it (`fn render_into(&self, out: &mut Vec<u8>)`), with an allocating
  convenience wrapper on top — so callers in a hot loop can reuse one buffer.
- **Inline storage**: `SmallVec` / `ArrayVec` keep small collections on the stack.
- **Swap the global allocator** for multithreaded throughput when profiling shows
  allocator contention: `#[global_allocator]` with `mimalloc` or `jemalloc`.

The principle: match allocator lifetime to data lifetime, and never allocate in the
hot loop when a reused buffer or arena will do.

---

## 7. Concurrency and parallelism in Rust

The headline is a genuinely negative-cost correctness abstraction: **`Send` and `Sync`
encode thread-safety in the type system, so data races are compile errors, not
undefined behavior.** You get the guarantee for free at runtime — the checks are the
compiler's, not the program's.

- **Scoped threads (std, stable):** `std::thread::scope` lets threads borrow local data
  safely because the scope guarantees they finish before the borrow ends.
- **Data parallelism — Rayon.** Swap `iter()` for `par_iter()` (or `par_chunks_mut`) and
  a work-stealing pool runs it across cores:

```rust
use rayon::prelude::*;
// Partition, don't share: each thread gets a disjoint &mut slice — no locks, no
// false sharing across chunks. This is the SoA/data-parallel payoff.
positions.par_chunks_mut(4096)
    .zip(velocities.par_chunks(4096))
    .for_each(|(ps, vs)| for (p, v) in ps.iter_mut().zip(vs) { *p += *v; });
```

- **Shared state when irreducible:** `Arc<Mutex<T>>` / `RwLock` for coordination;
  `AtomicUsize` etc. with an explicit `Ordering` for lock-free counters/flags. Reach down
  this ladder only for the coordination that can't be partitioned away.
- **False sharing:** wrap hot per-thread fields in `crossbeam_utils::CachePadded` so they
  land on separate cache lines.
- **Channels:** `std::sync::mpsc` or the faster `crossbeam-channel`.

Contrast with Zig: Rust selects its concurrency mechanism at the _ecosystem_ level
(Rayon for CPU-bound data parallelism, an async runtime like Tokio for I/O concurrency),
whereas Zig 0.16 unifies both behind the passed-in `Io`. Rust's compensating advantage
is that `Send`/`Sync` make the safety static.

---

## 8. Type-driven design in Rust (where Rust shines hardest)

This is Rust's home turf — the type system is expressive enough to make most illegal
states unrepresentable.

- **Sum types over flag-bags.** Model mutually exclusive states as an `enum`, not a
  struct of `bool`s whose illegal combinations you'd otherwise guard at runtime.
- **Parse, don't validate.** Validate once at the boundary into a refined newtype that
  can only be constructed through the validating path; the rest of the program takes the
  refined type and never re-checks:

```rust
pub struct Email(String);                 // field private: the invariant lives here
impl Email {
    pub fn parse(s: &str) -> Result<Email, ParseError> { /* validate once */ }
    pub fn as_str(&self) -> &str { &self.0 }
}
// Functions take `Email`, not `&str` — an unvalidated address can't reach them.
```

- **Newtypes** for IDs, units, and indices (`index_vec` gives checked typed indices) so
  they can't be confused or misused arithmetically.
- **Niche optimization = negative-cost correctness.** `NonZeroU32` forbids zero, so the
  compiler reuses the zero pattern as the niche: `Option<NonZeroU32>` is the _same size_
  as `u32`. You get the invariant and a smaller type. (`NonZero*`, `NonNull`, enums with
  unused discriminants all benefit.)
- **Typestate** (the `File<Open>`/`File<Closed>` pattern from §1's zero-cost section)
  encodes a state machine in the types, with illegal transitions rejected at compile
  time and zero runtime footprint.
- **Sealed traits** and `#[non_exhaustive]` to control how downstream code can extend or
  match your types.

The payoff matches the theory doc: validity proven once, defensive checks deleted
downstream, bugs surfaced at compile time — and frequently a smaller representation.

---

## 9. SIMD in Rust

Vectorizing a hot loop is the simd-loops skill's job — its [rust.md](../simd-loops/rust.md) carries the `std::simd` API table, the stable-Rust options, and the target-feature gotchas.
