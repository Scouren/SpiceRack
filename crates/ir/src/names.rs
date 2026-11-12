//! The name interner and the text pool: `&str` in, `NameId`/`TextId` out, and back.
//!
//! Names are exact bytes (C6): the interner never folds case, trims or normalizes; a reader of a
//! case-folding dialect hands it one spelling per name. Ids are given in first-seen order and
//! `"0"` is interned first, so [`NameId::GROUND`] is id 0. Text (`Opaque`, `Comment`, `Title`
//! bodies) is appended, never interned: two equal comments are two rows.
//!
//! Layout: one byte buffer, one end offset per entry; the interner's lookup map is keyed by the
//! 64-bit hash of the bytes and chains collisions through `next` (4 bytes per name, no second
//! copy of the bytes). The map is for lookup only; nothing iterates it (determinism, §1).
//!
//! Cites: `rewrite/IR-KERNEL.md` C6, C10; `compiler-construction` rule 2.
//!
//! Contents:
//! - `pub(crate) struct Names`: `Names::new() -> Names`, `Names::intern(&mut self, &str) -> NameId`,
//!   `Names::get(&self, &str) -> NameId`, `Names::str(&self, NameId) -> &str`,
//!   `Names::len(&self) -> usize`, `Names::is_empty(&self) -> bool`
//! - `pub(crate) struct Texts`: `Texts::new() -> Texts`, `Texts::push(&mut self, &str) -> TextId`,
//!   `Texts::str(&self, TextId) -> &str`, `Texts::len(&self) -> usize`

use std::collections::HashMap;

#[cfg(doc)]
use crate::id::{BYTES_MAX, NAMES_MAX};
use crate::id::{NameId, TextId};

/// The interner. Invariant: `ends.len() == next.len()`, `ends` is non-decreasing, and entry `k`
/// is `bytes[ends[k-1]..ends[k]]` (`ends[-1]` = 0).
#[derive(Clone, Debug)]
pub(crate) struct Names {
    /// Every name's bytes, back to back.
    pub(crate) bytes: String,
    /// End offset of each name in `bytes`.
    pub(crate) ends: Vec<u32>,
    /// Hash of the bytes → the most recently interned name with that hash.
    pub(crate) buckets: HashMap<u64, NameId>,
    /// Per name: the previously interned name with the same hash, or `NONE`.
    pub(crate) next: Vec<NameId>,
}

/// The text pool. Same layout as [`Names`] without the lookup.
#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct Texts {
    /// Every text's bytes, back to back.
    pub(crate) bytes: String,
    /// End offset of each text in `bytes`.
    pub(crate) ends: Vec<u32>,
}

impl Names {
    /// An interner holding only `"0"` (id 0, [`NameId::GROUND`]).
    pub(crate) fn new() -> Names {
        todo!()
    }

    /// The id of `name`, interning it when new. Asserts `name` is non-empty, and the
    /// [`NAMES_MAX`] and [`BYTES_MAX`] capacities.
    pub(crate) fn intern(&mut self, name: &str) -> NameId {
        todo!()
    }

    /// The id of `name` if it was interned, else `NONE`. Never interns.
    pub(crate) fn get(&self, name: &str) -> NameId {
        todo!()
    }

    /// The bytes of `id`. Asserts `id` is in range (not `NONE`).
    pub(crate) fn str(&self, id: NameId) -> &str {
        todo!()
    }

    /// Interned name count (at least 1: ground).
    pub(crate) fn len(&self) -> usize {
        todo!()
    }

    /// Always false: ground is interned at construction. Here for clippy's `len` rule.
    pub(crate) fn is_empty(&self) -> bool {
        todo!()
    }
}

impl Texts {
    /// An empty pool.
    pub(crate) fn new() -> Texts {
        todo!()
    }

    /// Appends `text` (may be empty) and returns its id. Asserts [`BYTES_MAX`].
    pub(crate) fn push(&mut self, text: &str) -> TextId {
        todo!()
    }

    /// The bytes of `id`. Asserts `id` is in range.
    pub(crate) fn str(&self, id: TextId) -> &str {
        todo!()
    }

    /// Text count.
    pub(crate) fn len(&self) -> usize {
        todo!()
    }
}
