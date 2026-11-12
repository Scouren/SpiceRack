//! Provenance: where an item came from (file, byte range), kept in the cold half of every item
//! table and in diagnostic rows. Used only to anchor diagnostics: no reader, writer or pass
//! decides anything by a span, and structural equality ignores spans (§3.1 principle 6).
//!
//! Cites: `REWRITE-PLAN.md` §3.1 (6), §3.2 (`spans`); `rewrite/IR-KERNEL.md` C4, C14.
//!
//! Contents: `pub struct Span`, `Span::NONE`, `Span::is_none(self) -> bool`

use crate::id::FileId;

/// A byte range of one source file. Built items (Python, Rust builders, passes) carry
/// [`Span::NONE`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Span {
    /// The file the bytes are in; `NONE` for an item with no source.
    pub file: FileId,
    /// First byte, from the start of the file.
    pub start: u32,
    /// Byte count.
    pub len: u32,
}

const _: () = assert!(size_of::<Span>() == 12);

impl Span {
    /// No source: file `NONE`, empty range at 0.
    pub const NONE: Span = Span {
        file: FileId::NONE,
        start: 0,
        len: 0,
    };

    /// Whether the span has no file.
    pub const fn is_none(self) -> bool {
        todo!()
    }
}
