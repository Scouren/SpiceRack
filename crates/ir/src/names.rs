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
//!   `Names::intern_any(&mut self, &str) -> NameId`, `Names::get(&self, &str) -> NameId`, `Names::str(&self, NameId) -> &str`,
//!   `Names::len(&self) -> usize`, `Names::is_empty(&self) -> bool`
//! - `pub(crate) struct Texts`: `Texts::new() -> Texts`, `Texts::push(&mut self, &str) -> TextId`,
//!   `Texts::str(&self, TextId) -> &str`, `Texts::len(&self) -> usize`

use std::collections::HashMap;
use std::hash::{DefaultHasher, Hash, Hasher};

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
        let mut names = Names {
            bytes: String::new(),
            ends: Vec::new(),
            buckets: HashMap::new(),
            next: Vec::new(),
        };
        let ground = names.intern("0");
        assert!(ground == NameId::GROUND, "ground is the first name");
        names
    }

    /// The id of `name`, interning it when new. Asserts `name` is non-empty, and the
    /// [`NAMES_MAX`] and [`BYTES_MAX`] capacities.
    pub(crate) fn intern(&mut self, name: &str) -> NameId {
        assert!(!name.is_empty(), "a name is non-empty");
        self.intern_any(name)
    }

    /// The id of `name`, interning it when new, empty included (a `Str` value may be `""`;
    /// only names are non-empty). Asserts the capacities.
    pub(crate) fn intern_any(&mut self, name: &str) -> NameId {
        let found = self.get(name);
        if !found.is_none() {
            return found;
        }
        assert!(self.ends.len() < NAMES_MAX, "at most NAMES_MAX names");
        assert!(
            self.bytes.len() + name.len() <= BYTES_MAX,
            "name bytes within BYTES_MAX"
        );
        let id = NameId(self.ends.len() as u32);
        self.bytes.push_str(name);
        self.ends.push(self.bytes.len() as u32);
        let older = self.buckets.insert(hash(name), id).unwrap_or(NameId::NONE);
        self.next.push(older);
        debug_assert_eq!(self.ends.len(), self.next.len(), "parallel columns");
        id
    }

    /// The id of `name` if it was interned, else `NONE`. Never interns.
    pub(crate) fn get(&self, name: &str) -> NameId {
        let mut at = self
            .buckets
            .get(&hash(name))
            .copied()
            .unwrap_or(NameId::NONE);
        // bounded by the name count: each step moves to an older name
        for _ in 0..=self.ends.len() {
            if at.is_none() || self.str(at) == name {
                return at;
            }
            at = self.next[at.index()];
        }
        unreachable!("a collision chain is acyclic")
    }

    /// The bytes of `id`. Asserts `id` is in range (not `NONE`).
    pub(crate) fn str(&self, id: NameId) -> &str {
        let i = id.index();
        assert!(i < self.ends.len(), "a name id in range");
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        &self.bytes[start..self.ends[i] as usize]
    }

    /// Interned name count (at least 1: ground).
    pub(crate) fn len(&self) -> usize {
        self.ends.len()
    }

    /// Always false: ground is interned at construction. Here for clippy's `len` rule.
    pub(crate) fn is_empty(&self) -> bool {
        self.ends.is_empty()
    }
}

impl Texts {
    /// An empty pool.
    pub(crate) fn new() -> Texts {
        Texts::default()
    }

    /// Appends `text` (may be empty) and returns its id. Asserts [`BYTES_MAX`].
    pub(crate) fn push(&mut self, text: &str) -> TextId {
        assert!(
            self.bytes.len() + text.len() <= BYTES_MAX,
            "text bytes within BYTES_MAX"
        );
        assert!(self.ends.len() < u32::MAX as usize, "a text id fits u32");
        self.bytes.push_str(text);
        self.ends.push(self.bytes.len() as u32);
        TextId((self.ends.len() - 1) as u32)
    }

    /// The bytes of `id`. Asserts `id` is in range.
    pub(crate) fn str(&self, id: TextId) -> &str {
        let i = id.index();
        assert!(i < self.ends.len(), "a text id in range");
        let start = if i == 0 { 0 } else { self.ends[i - 1] as usize };
        &self.bytes[start..self.ends[i] as usize]
    }

    /// Text count.
    pub(crate) fn len(&self) -> usize {
        self.ends.len()
    }
}

// ---- private ----

/// The lookup key of a name's bytes (SipHash with fixed keys: deterministic across runs).
fn hash(name: &str) -> u64 {
    let mut h = DefaultHasher::new();
    name.hash(&mut h);
    h.finish()
}
