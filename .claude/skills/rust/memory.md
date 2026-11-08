# Memory, errors, unsafe

Reference for [`SKILL.md`](SKILL.md) step 2. `data-oriented-design/rust.md` §5–6 already covers the basics: `Option` versus `Result`, `?`, the `thiserror`/`anyhow` split, `with_capacity`, `clear()` to reuse a buffer, `bumpalo`, caller-owned output, and swapping the global allocator. This file adds the house rules and the measured numbers.

## Graphs are a `Vec` plus indices

A graph of nodes (a netlist, an AST, a scene) is one `Vec<Node>` per node kind, with edges stored as typed index handles ([`types.md`](types.md)). It is not `Rc<RefCell<Node>>`. The `Rc` version pays one heap allocation per node, two reference counts and a borrow flag per node, a pointer chase per edge, and a runtime borrow check that can panic. The `Vec` version is one allocation per table, is freed at a phase boundary, and its edges are `Copy` integers that serialize as they are.

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExprId(u32);

#[derive(Debug)]
pub enum Expr {
    Num(f64),
    Add(ExprId, ExprId),
    Mul(ExprId, ExprId),
}

#[derive(Default)]
pub struct Exprs {
    nodes: Vec<Expr>,
}

impl Exprs {
    pub fn with_capacity(n: usize) -> Self {
        Exprs { nodes: Vec::with_capacity(n) }
    }

    pub fn push(&mut self, e: Expr) -> ExprId {
        let id = u32::try_from(self.nodes.len()).expect("invariant: fewer than 2^32 expressions");
        self.nodes.push(e);
        ExprId(id)
    }

    /// Children are pushed before parents, so one forward pass evaluates everything.
    pub fn eval_all(&self, out: &mut Vec<f64>) {
        out.clear();
        out.reserve(self.nodes.len());
        for e in &self.nodes {
            let v = match *e {
                Expr::Num(x) => x,
                Expr::Add(a, b) => out[a.0 as usize] + out[b.0 as usize],
                Expr::Mul(a, b) => out[a.0 as usize] * out[b.0 as usize],
            };
            out.push(v);
        }
    }
}

#[test]
fn evaluates_in_push_order() {
    let mut xs = Exprs::with_capacity(4);
    let two = xs.push(Expr::Num(2.0));
    let three = xs.push(Expr::Num(3.0));
    let sum = xs.push(Expr::Add(two, three));
    xs.push(Expr::Mul(sum, sum));
    let mut out = Vec::new();
    xs.eval_all(&mut out);
    assert_eq!(out, [2.0, 3.0, 5.0, 25.0]);
}
```

Push order is a topological order for free, so "evaluate everything" becomes a forward loop instead of a recursive walk. For per-phase scratch that is not a table (strings built while parsing, a temporary tree), use a bump arena (`bumpalo`, `data-oriented-design/rust.md` §6) named for its phase. Keep `Rc` for data that is actually shared across owners whose lifetimes you cannot order. That case is rare in a pipeline.

## Sizing containers

- **`Vec::with_capacity(n)`** wherever `n` is known or bounded before the loop: the table's stated capacity, the input length, the previous frame's count. A `Vec` that grows from empty to `n` reallocates and copies about log₂ n times (19 times for 2²⁰ `f64` pushes on 1.95; the growth policy is not a documented guarantee).
- **`Box<[T]>` for a table that is built once and never grows.** It is 16 bytes (pointer and length) against `Vec`'s 24, and the type tells the reader the length is fixed. `Box<str>` is the same against `String` (16 against 24, measured on 1.95, x86-64). Convert with `into_boxed_slice()`/`into_boxed_str()`. That call shrinks the allocation to fit, which may reallocate once.
- **`Cow<'a, T>` when the common case needs no change.** Return the input borrowed and allocate only on the path that edits it:

```rust
use std::borrow::Cow;

/// SPICE names are case-insensitive; most inputs are already lower case.
pub fn fold_case(name: &str) -> Cow<'_, str> {
    if name.bytes().any(|b| b.is_ascii_uppercase()) {
        Cow::Owned(name.to_ascii_lowercase())
    } else {
        Cow::Borrowed(name)
    }
}

#[test]
fn borrows_when_unchanged() {
    assert!(matches!(fold_case("vdd"), Cow::Borrowed("vdd")));
    assert_eq!(fold_case("VDD"), "vdd");
}
```

## Errors: `thiserror` in libraries

A library crate returns one error enum per module boundary, derived with `thiserror` 2. Each variant names one failure the caller might handle differently, carries the data needed to report it, and chains its cause with `#[source]` or `#[from]`. `anyhow` belongs in binaries only. The reason is in `data-oriented-design/rust.md` §5.

```rust
use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum LoadError {
    #[error("reading {path}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("line {line}: unknown element `{tag}`")]
    UnknownElement { line: u32, tag: Box<str> },
    #[error("line {line}: {source}")]
    Number {
        line: u32,
        #[source]
        source: std::num::ParseFloatError,
    },
}

pub fn load(path: &std::path::Path) -> Result<Vec<f64>, LoadError> {
    let text = std::fs::read_to_string(path)
        .map_err(|source| LoadError::Io { path: path.to_owned(), source })?;
    let mut out = Vec::with_capacity(text.lines().count());
    for (i, l) in text.lines().enumerate() {
        let line = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let Some(rest) = l.strip_prefix("V ") else {
            return Err(LoadError::UnknownElement { line, tag: l.split(' ').next().unwrap_or("").into() });
        };
        out.push(rest.trim().parse().map_err(|source| LoadError::Number { line, source })?);
    }
    Ok(out)
}

#[test]
fn reports_line_and_cause() {
    let e: Result<f64, _> = "x".parse::<f64>();
    let err = LoadError::Number { line: 3, source: e.unwrap_err() };
    assert_eq!(err.to_string(), "line 3: invalid float literal");
    assert!(std::error::Error::source(&err).is_some());
}
```

Messages are lower case without a trailing period, so they compose when a caller wraps them. Use `#[from]` only when the conversion loses nothing. A bare `io::Error` with no path is a message nobody can act on, which is why `Io` above is built by hand.

## `unwrap` and `expect` state invariants

A panic in this codebase means a bug in this codebase. `expect("invariant: ...")` is for a condition the surrounding code has already guaranteed, and the message names that guarantee, so the panic tells the reader which proof failed. Everything the outside world can cause (input bytes, files, the user, a plugin) is a `Result`. `unwrap()` is acceptable in tests and in `const` contexts where failure is a compile error. In library code, prefer `expect` with the invariant spelled out. The `push` above is the pattern. Where the invariant is cheap to restate as an assertion at the transform's entry, assert it there, and the `expect` downstream becomes redundant.

## `unsafe`: forbidden except at FFI

Every crate sets `unsafe_code = "forbid"` in `[workspace.lints.rust]` and opts in with `[lints] workspace = true`. Two facts on Rust 1.95 decide how the exception works:

- `forbid` cannot be relaxed lower down. `#![forbid(unsafe_code)]` followed by `#[allow(unsafe_code)] mod ffi;` is error E0453.
- A crate cannot inherit the workspace lints and override one: `[lints] workspace = true` next to another key under `[lints]` fails to parse.

So FFI lives in its own crate (`foo-sys`, or the one crate that owns the foreign library). That crate copies the workspace lint table into its own `[lints]`, with `unsafe_code = "deny"` instead of `forbid`, `clippy::undocumented_unsafe_blocks = "deny"`, and `#[allow(unsafe_code)]` on the single `ffi` module. Every `unsafe` block carries a `// SAFETY:` comment naming the obligation and why it holds here (the clippy lint rejects a block without one). Each `unsafe fn` has a `# Safety` section in its doc comment (`doc-comments`).

```rust
#![deny(unsafe_code)]

#[allow(unsafe_code)]
mod ffi {
    unsafe extern "C" {
        safe fn abs(x: i32) -> i32;
        fn strlen(s: *const std::ffi::c_char) -> usize;
    }

    pub fn c_abs(x: i32) -> i32 {
        abs(x)
    }

    pub fn c_len(s: &std::ffi::CStr) -> usize {
        // SAFETY: `CStr` guarantees a valid, NUL-terminated pointer for the borrow's lifetime,
        // and strlen reads only up to that NUL.
        unsafe { strlen(s.as_ptr()) }
    }
}

pub use ffi::{c_abs, c_len};

#[test]
fn calls_libc() {
    assert_eq!(c_abs(-3), 3);
    assert_eq!(c_len(c"abc"), 3);
}
```

Edition 2024 requires `unsafe extern` blocks and lets you mark an item that has no preconditions `safe fn`, so callers need no `unsafe` for it. Outside FFI, "a profile shows the bounds check costs X" is a reason to restructure the loop (`chunks_exact`, slicing once, an `assert!` the optimizer can use) and re-probe. It is not a reason to reach for `get_unchecked` (Schemify `contributing.md` rule 6).
