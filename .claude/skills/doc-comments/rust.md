# Doc comments in Rust

Rust does much of the ladder's work for you. Invalidation and ownership, ranks 2 and 3 that dominate the Zig file, are enforced by the borrow checker and rarely need prose. What is left is what the type system cannot say: when it fails, what panics, what an `unsafe` caller must guarantee, what it costs, and why.

Everything below was checked with Rust 1.95.0 and a scratch crate: `cargo doc` printed zero warnings, the doctests passed under `cargo test`, and `cargo clippy` was clean.

## Sections, in order

The Rust API Guidelines (C-FAILURE) ask for `# Errors` on every function that may fail, `# Panics` on every function that may panic, and `# Safety` on every `unsafe fn`. `# Examples` is C-EXAMPLE.

| Section | When | What it states |
|---|---|---|
| `# Errors` | every function returning `Result` | each error variant and the condition that produces it, not only the type |
| `# Panics` | every reachable `assert!`, `unwrap`, index, or overflow the caller can trigger | the caller-side condition, in the caller's terms |
| `# Safety` | every `unsafe fn` and `unsafe trait` | what the caller must guarantee; never what the function does with it |
| `# Examples` | public items | a doctest, using `?` |
| `# Time complexity` | when cost is not O(1) or not what the name implies | std's own section name; adopt it per project and stay consistent |

`Vec::push` in 1.95 is the pattern: the summary is trivial and the value sits in the sections.

```rust
/// Appends an element to the back of a collection.
///
/// # Panics
///
/// Panics if the new capacity exceeds `isize::MAX` _bytes_.
///
/// # Examples
///
/// ```
/// let mut vec = vec![1, 2];
/// vec.push(3);
/// assert_eq!(vec, [1, 2, 3]);
/// ```
///
/// # Time complexity
///
/// Takes amortized *O*(1) time. If the vector's length would exceed its
/// capacity after the push, *O*(*capacity*) time is taken to copy the
/// vector's elements to a larger allocation. This expensive operation is
/// offset by the *capacity* *O*(1) insertions it allows.
```

`# Safety` is the section the compiler enforces least: the language demands `unsafe`, never the obligation behind it. Pair it with a `// SAFETY:` comment at every call site that says why the obligation holds there.

### Domain sections are legitimate

When a domain has a recurring invariant no type expresses, name a section for it. Tokio's `# Cancel safety` on async methods is the model (`mpsc::Sender::send`):

> If `send` is used as a branch in `tokio::select!` and another branch completes first, then it is guaranteed that the message was not sent. **However, in that case, the message is dropped and will be lost.**

A recurring named section beats the same caveat rephrased in prose across forty methods, and it makes an omission visible.

## The crate and module header

The `//!` at the top of `lib.rs` (and of each public module) carries the same two facts as a Zig file header: in → out, and what the code cites. Schemify's `crates/doc/src/lib.rs` opens with the decision records it implements, the one thing it owns, what it is (`Pure: bytes in, bytes and requests out`), and the normative spec sections. C-CRATE-DOC asks for crate docs with an example; put a doctest in the header.

## Examples are tests

`cargo test` compiles and runs every doctest, so the example is the one part of a doc comment that cannot rot silently, provided it runs.

- Use `?`, not `unwrap` or `expect` (C-QUESTION-MARK): users copy examples verbatim. End the example with a hidden `# Ok::<(), YourError>(())` line so `?` compiles.
- Hide setup lines with a leading `# ` rather than deleting them; the example still compiles and the reader sees only what matters.
- ` ```ignore ` and ` ```text ` opt out of compilation. An example that cannot run is a hint that it carries no fact.

## Intra-doc links

Link every type, method, constant and error variant named in prose: `` [`split`] ``, `` [`SplitError::TooManyLines`] `` (C-LINK). With `rustdoc::broken_intra_doc_links` denied, a renamed item breaks `cargo doc` instead of leaving a dead link. Verified: a link to a misspelt constant fails with `error: unresolved link to 'LINE_MAX'`.

## Lints

Set them once in `Cargo.toml`, in the workspace table when there is a workspace (Schemify sets `[workspace.lints.rust] missing_docs = "warn"`, and each crate opts in with `[lints] workspace = true`):

```toml
[lints.rust]
missing_docs = "warn"
unsafe_op_in_unsafe_fn = "deny"

[lints.rustdoc]
broken_intra_doc_links = "deny"

[lints.clippy]
missing_errors_doc = "warn"
missing_panics_doc = "warn"
missing_safety_doc = "warn"
```

What each one catches, verified by deleting the matching doc from the crate below:

| Lint | Fires in | Default | Message |
|---|---|---|---|
| `missing_docs` | `cargo build` and `cargo doc` | allow | `missing documentation for a struct` |
| `rustdoc::broken_intra_doc_links` | `cargo doc` only | warn | `unresolved link to ...` |
| `clippy::missing_errors_doc` | `cargo clippy` | off (pedantic) | ``docs for function returning `Result` missing `# Errors` section`` |
| `clippy::missing_panics_doc` | `cargo clippy` | off (pedantic) | ``docs for function which may panic missing `# Panics` section`` |
| `clippy::missing_safety_doc` | `cargo clippy` | **warn** | ``unsafe function's docs are missing a `# Safety` section`` |

Only `missing_safety_doc` fires without the table; the other two must be turned on. `rustdoc::` lints run only under rustdoc, so a CI job that never builds docs never checks links.

The gate:

```sh
cargo test                                   # unit tests and doctests
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps   # zero warnings, or the job fails
cargo clippy -- -D warnings
```

Schemify's definition of done says the same: `cargo doc -p <crate>` prints no warnings.

`clippy::missing_docs_in_private_items` stays off: private helpers are on the skill's *write none* list.

## A complete crate

`src/lib.rs` of the scratch crate, with the `Cargo.toml` above. `cargo doc` prints no warnings, `cargo test` passes all three doctests, `cargo clippy` is clean:

````rust
//! Line splitter: a netlist's text in, one [`Line`] per logical line out,
//! with `+` continuations joined and `*` comment lines dropped.
//!
//! Rows are byte ranges into the input; nothing is copied.
//!
//! ```
//! let lines = rdoc::split("R1 a b\n+ 1k\nC1 b 0 1p")?;
//! assert_eq!(lines.len(), 2);
//! # Ok::<(), rdoc::SplitError>(())
//! ```

use std::fmt;

/// Most lines one netlist may hold. A deck past this is refused, not truncated.
pub const LINES_MAX: usize = 1 << 20;

/// One logical line: a byte range of the input passed to [`split`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Line {
    /// First byte of the line.
    pub start: u32,
    /// One past the last byte, continuations included.
    pub end: u32,
}

/// Why [`split`] refused its input.
#[derive(Debug, PartialEq, Eq)]
pub enum SplitError {
    /// A `+` continuation came before any line it could continue.
    OrphanContinuation,
    /// The input holds more than [`LINES_MAX`] logical lines.
    TooManyLines,
}

impl fmt::Display for SplitError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::OrphanContinuation => "continuation line before any line",
            Self::TooManyLines => "too many lines",
        })
    }
}

impl std::error::Error for SplitError {}

/// Splits `text` into logical lines.
///
/// # Errors
///
/// Returns [`SplitError::OrphanContinuation`] if the first non-comment line
/// starts with `+`, and [`SplitError::TooManyLines`] past [`LINES_MAX`] lines.
///
/// # Panics
///
/// Panics if `text` is 4 GiB or longer, since rows store `u32` offsets.
///
/// # Examples
///
/// ```
/// use rdoc::{split, SplitError};
///
/// let lines = split("* header\nR1 a b\n+ 1k")?;
/// assert_eq!(lines[0].start, 9);
/// assert_eq!(split("+ 1k"), Err(SplitError::OrphanContinuation));
/// # Ok::<(), SplitError>(())
/// ```
pub fn split(text: &str) -> Result<Vec<Line>, SplitError> {
    assert!(u32::try_from(text.len()).is_ok(), "text exceeds u32 offsets");
    let mut out: Vec<Line> = Vec::new();
    let mut start = 0usize;
    for raw in text.split('\n') {
        let end = start + raw.len();
        let span = Line { start: start as u32, end: end as u32 };
        start = end + 1;
        if raw.is_empty() || raw.starts_with('*') {
            continue;
        }
        if raw.starts_with('+') {
            let last = out.last_mut().ok_or(SplitError::OrphanContinuation)?;
            last.end = span.end;
            continue;
        }
        if out.len() == LINES_MAX {
            return Err(SplitError::TooManyLines);
        }
        out.push(span);
    }
    Ok(out)
}

/// Returns the line at `index` without a bounds check.
///
/// # Safety
///
/// `index` must be less than `lines.len()`.
///
/// # Examples
///
/// ```
/// let lines = rdoc::split("R1 a b")?;
/// // SAFETY: `split` returned one line, so index 0 is in bounds.
/// let first = unsafe { rdoc::line_unchecked(&lines, 0) };
/// assert_eq!(first.end, 6);
/// # Ok::<(), rdoc::SplitError>(())
/// ```
pub unsafe fn line_unchecked(lines: &[Line], index: usize) -> Line {
    // SAFETY: the caller guarantees `index < lines.len()`.
    unsafe { *lines.get_unchecked(index) }
}
````

Each `///` names a fact: the capacity and what happens past it, which input each variant means, the panic condition and its reason, the `unsafe` obligation. The `Line` field docs exist because `missing_docs` requires them; they earn it by saying what `end` includes.

## Keep out

Version history belongs in release notes, not in `///` (C-RELNOTES). Items you do not want documented get `#[doc(hidden)]` or `pub(crate)` rather than a comment explaining they are internal (C-HIDDEN).

Sources: Rust API Guidelines, Documentation chapter (rust-lang.github.io/api-guidelines/documentation.html); `Vec::push` docs, Rust 1.95.0 (doc.rust-lang.org/1.95.0/std/vec/struct.Vec.html); tokio `mpsc::Sender::send` docs (docs.rs/tokio); Schemify `Cargo.toml`, `crates/doc/src/lib.rs`, `docs/dev/contributing.md`.
