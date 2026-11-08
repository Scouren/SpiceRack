# Rust companion

Rust 1.95, edition 2024. The file below compiles with no warnings and its test passes under `rustc --edition 2024 --test`.

## A file that passes

Header, `use`, public types, public functions, then the `impl` blocks holding public methods and trait impls, the separator, private functions and the private `impl` block, tests last. `classify` and `run_end` are extract rule 2.

```rust
//! Tokenizer: source bytes → `Tokens` (one tag and one start offset per run).
//! Cites: none (house grammar, docs/grammar.md).
//!
//! Contents:
//! - `pub enum Tag`
//! - `pub struct Tokens`, `Tokens::tags(&self) -> &[Tag]`, `Tokens::starts(&self) -> &[u32]`
//! - `pub enum LexError`
//! - `pub fn tokenize(src: &[u8], out: &mut Tokens) -> Result<(), LexError>`

use std::fmt;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Tag {
    Ident,
    Number,
    Space,
    Punct,
}

/// One entry per token. Fields are private because `tags` and `starts` always have equal length.
#[derive(Default)]
pub struct Tokens {
    tags: Vec<Tag>,
    starts: Vec<u32>,
}

#[derive(Debug)]
pub enum LexError {
    /// The source is longer than `u32::MAX` bytes; `out` is untouched.
    SourceTooLong,
}

/// Appends one token per run of same-tag bytes.
pub fn tokenize(src: &[u8], out: &mut Tokens) -> Result<(), LexError> {
    let len = u32::try_from(src.len()).map_err(|_| LexError::SourceTooLong)?;
    let mut i = 0;
    while i < len {
        let tag = classify(src[i as usize]);
        let end = run_end(src, i, tag);
        out.tags.push(tag);
        out.starts.push(i);
        i = end;
    }
    debug_assert_eq!(out.tags.len(), out.starts.len());
    Ok(())
}

impl Tokens {
    pub fn tags(&self) -> &[Tag] {
        &self.tags
    }

    pub fn starts(&self) -> &[u32] {
        &self.starts
    }
}

impl fmt::Display for LexError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("source longer than u32::MAX bytes")
    }
}

// ---- private ----

fn classify(c: u8) -> Tag {
    match c {
        b'a'..=b'z' | b'A'..=b'Z' | b'_' => Tag::Ident,
        b'0'..=b'9' => Tag::Number,
        b' ' | b'\t' | b'\n' => Tag::Space,
        _ => Tag::Punct,
    }
}

fn run_end(src: &[u8], start: u32, tag: Tag) -> u32 {
    let mut i = start + 1;
    while (i as usize) < src.len() && classify(src[i as usize]) == tag {
        i += 1;
    }
    i
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_runs_by_tag() {
        let mut tokens = Tokens::default();
        tokenize(b"ab 12", &mut tokens).unwrap();
        assert_eq!(tokens.tags(), [Tag::Ident, Tag::Space, Tag::Number]);
        assert_eq!(tokens.starts(), [0, 2, 3]);
    }
}
```

## Methods and `impl` blocks

Rust allows any number of `impl` blocks per type, which is what makes the split work: the public methods go in an `impl` above the separator, and the private ones in a second `impl` of the same type below it. Trait impls (`Display`, `From`, `Iterator`) are public surface and sit above the separator.

Inside an `impl`, the public methods are in call order, constructor (`new`, `with_capacity`) first.

## Scope in Rust

The borrow checker writes the scope rule from SKILL.md into the types:

- An owned field (`Vec<T>`, `Box<T>`) means the struct holds that memory for its whole life. The field is right only when its last read is near the struct's drop.
- A borrowed field (`&'a [T]`) makes the struct a view that cannot leave the block holding the borrow. That is the right shape for a short-lived bundle of a phase's inputs, built in the spine and dropped with it.
- A field that fights the borrow checker is mis-scoped. Pass it as a parameter to the calls that read it; `Rc`, `Arc`, `'static`, or a `.clone()` added to quiet the checker keeps the memory alive past its last read.
- To free a phase's data at the phase's end, let it go out of scope there (a block `{ ... }` around the phase) or call `drop(tokens)` on the line after its last read.

## Crates

A crate is the unit Cargo compiles in parallel today, but a crate per owner multiplies public interfaces. A new crate is justified only by HIERARCHY.md's three reasons (enforce single ownership, gate a heavy dependency, split by platform); everything else is a module file inside an existing crate.
