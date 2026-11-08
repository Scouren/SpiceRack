# Types

Reference for [`SKILL.md`](SKILL.md) step 1. A type in this codebase does one of two jobs: it carries a **boundary** proof (this value was parsed, this index is in range, this state was reached), or it gets out of the way. Every rule below is the cheapest way to do the first job; [`zero-cost.md`](zero-cost.md) is how you prove it left no residue.

Sum types over flag-bags, sealed traits, and the `Email` parse example are already in `data-oriented-design/rust.md` §8. Typestate for init order and the "not on the fine tier" rule are in `api-design/rust.md`. This file adds the measured layouts and the house rules.

## Newtypes are `#[repr(transparent)]`

A unit, an ID, or a count gets its own type, and the wrapper is `#[repr(transparent)]` so its layout and ABI are the field's. That makes it safe to pass across `extern "C"` and to reinterpret a `&[f64]` as a `&[Volts]` in FFI code. Probe on Rust 1.95, x86-64-v3: a loop over `&mut [Volts]` is **FOLDED** with the same loop over `&mut [f64]`. LLVM emitted one body for both symbols.

```rust
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Volts(pub f64);

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Amps(pub f64);

pub fn power_w(v: Volts, i: Amps) -> f64 {
    v.0 * i.0 // `Volts * Volts` does not compile; mixing units needs a named function
}

const _: () = assert!(size_of::<Volts>() == size_of::<f64>());
```

Implement only the operators that are dimensionally true (`Volts + Volts`, not `Volts * Volts`). The field is `pub` when the type carries no invariant. It is private when it does: then the type is a parsed value (see below).

## Index handles, not references

A cross-table reference is a typed index into the table that owns the row (`codebase-clean-code` rule 6). The type stops a `NodeId` from indexing `pins`.

```rust
use std::num::NonZero;

#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

/// An optional link: stores `index + 1`, so `Option<NetId>` is still 4 bytes.
#[repr(transparent)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NetId(NonZero<u32>);

impl NetId {
    pub fn from_index(i: usize) -> Option<NetId> {
        let raw = u32::try_from(i).ok()?.checked_add(1)?;
        NonZero::new(raw).map(NetId)
    }
    pub fn index(self) -> usize {
        self.0.get() as usize - 1
    }
}

pub struct Nodes {
    pub x: Vec<i32>,
    pub net: Vec<Option<NetId>>,
}

impl Nodes {
    pub fn x(&self, id: NodeId) -> i32 {
        self.x[id.0 as usize]
    }
}

const _: () = assert!(size_of::<Option<NetId>>() == 4);
```

Pick the representation by whether the handle is ever absent. Probes on 1.95, x86-64-v3:

| Handle | Versus | Verdict |
|---|---|---|
| `NodeId(u32)` indexing a slice | raw `u32` index | EQUIVALENT (same instructions, different registers) |
| `NetId(NonZero<u32>)`, always present | raw `u32` index | RESIDUE: one extra instruction (the `- 1`) per lookup |
| `Option<NetId>` | `u32` with 0 as the "none" sentinel | EQUIVALENT |

So use plain `u32` handles for links that always exist, and `NonZero` handles where the field is `Option`. Add a generation counter (`slotmap`, in `data-oriented-design/rust.md`) only when a slot can be freed and reused while a handle to it survives.

## Niches

The compiler stores `None` in a bit pattern the payload can never hold. Measured on 1.95:

| Type | Size (bytes) |
|---|---|
| `Option<u32>` | 8 |
| `Option<NonZero<u32>>` | 4 |
| `Option<&T>`, `Option<Box<T>>`, `Option<NonNull<T>>` | 8 |
| `Option<char>` | 4 |
| `Option<Kind>` for a fieldless `#[repr(u8)]` enum with 3 variants | 1 |

A table of a million `Option<u32>` is 8 MB. As `Option<NonZero<u32>>` it is 4 MB. Pin the size of every type that sits in a large table with a `const _: () = assert!(size_of::<T>() == N);` next to its definition, so growth fails the build instead of the cache.

## Closed enums, exhaustive matches

Variation that is a little behaviour over a few kinds is a closed `enum` and an exhaustive `match` (`codebase-clean-code` rule 4). The match lists every variant, grouping with `|` where arms share a body, so adding a variant turns every site that must handle it into a compile error.

A **boundary enum** is one whose values come from outside the program: a file format, a wire protocol, a CLI flag, a plugin ABI. Its `match` has no `_` arm. A `_` there silently routes a newly added variant into whatever the catch-all does, and that is how a new file-format tag gets written out as the wrong thing. A `_` arm is correct in one place only: matching a foreign `#[non_exhaustive]` enum, where the language requires it. Give that arm a comment saying so.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Kind {
    Wire = 0,
    Pin = 1,
    Label = 2,
}

impl Kind {
    /// The boundary: a tag byte from the file becomes a `Kind` or an error, once.
    pub fn from_tag(tag: u8) -> Result<Kind, u8> {
        match tag {
            0 => Ok(Kind::Wire),
            1 => Ok(Kind::Pin),
            2 => Ok(Kind::Label),
            other => Err(other), // integers are open; the catch-all here is the rejection
        }
    }
    pub fn tag(self) -> u8 {
        self as u8
    }
}

pub fn is_connectable(k: Kind) -> bool {
    match k {
        Kind::Wire | Kind::Pin => true,
        Kind::Label => false,
    }
}

const _: () = assert!(size_of::<Option<Kind>>() == 1);
```

Within a workspace, `#[non_exhaustive]` on your own enum has no effect inside its defining crate and forces every other crate in the workspace to write `_`. Leave it off enums whose consumers you control.

## Typestate at boundaries only

A typestate (`Builder<Unbound>` to `Builder<Bound>`, `PhantomData` markers) earns its place where the caller crosses an API boundary and a wrong order would otherwise be a runtime panic. Inside a crate, between your own transforms, the spine already fixes the order. There, a typestate is generics and impl blocks paid for a proof that the spine's single call site already gives. The markers are zero-sized, so the cost is compile time and readability, not bytes:

```rust
use std::marker::PhantomData;

pub struct Unbound;
pub struct Bound;

pub struct Session<S> {
    fd: i32,
    _s: PhantomData<S>,
}

impl Session<Unbound> {
    pub fn new(fd: i32) -> Self {
        Session { fd, _s: PhantomData }
    }
    pub fn bind(self, port: u16) -> Session<Bound> {
        let _ = port;
        Session { fd: self.fd, _s: PhantomData }
    }
}

impl Session<Bound> {
    pub fn fd(&self) -> i32 {
        self.fd
    }
}

const _: () = assert!(size_of::<Session<Bound>>() == size_of::<i32>());
```

## Parse, don't validate

Every untrusted value crosses the boundary once, through a function that returns either a refined type or an error. Everything downstream takes the refined type and checks nothing again. The test is in the signatures: a function deep in the core that takes `&str`, `u32`, or `f64` for something with rules is a validation waiting to be forgotten.

```rust
use std::num::NonZero;

/// A SPICE node name: non-empty ASCII, no whitespace, at most 64 bytes.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct NodeName(Box<str>);

#[derive(Debug, PartialEq, Eq)]
pub enum NodeNameError {
    Empty,
    TooLong(usize),
    BadByte(u8),
}

impl NodeName {
    pub fn parse(s: &str) -> Result<NodeName, NodeNameError> {
        if s.is_empty() {
            return Err(NodeNameError::Empty);
        }
        if s.len() > 64 {
            return Err(NodeNameError::TooLong(s.len()));
        }
        if let Some(&b) = s.as_bytes().iter().find(|b| !b.is_ascii_graphic()) {
            return Err(NodeNameError::BadByte(b));
        }
        Ok(NodeName(s.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A step count the caller cannot make zero.
pub fn steps(n: NonZero<u32>, span_ns: u64) -> u64 {
    span_ns / u64::from(n.get()) // no divide-by-zero branch: the type already excluded it
}
```

Use `NonZero<T>`, `Box<str>` instead of `String` for a value that never grows, and a private field with a single `parse` constructor. Implement `TryFrom` or `FromStr` on top of `parse` when a caller wants `?`/`.parse()` ergonomics. The errors are a typed enum. Library error types use `thiserror` (see [`memory.md`](memory.md)).
