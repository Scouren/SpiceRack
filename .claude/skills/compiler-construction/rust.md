# Rust 1.95 layout

Open this when you are writing the row layout, intern pool, fact table, diagnostic catalogue or a worklist walk for a Rust frontend. The file below is `src/lib.rs` of a crate with `edition = "2024"` and `hashbrown = "0.17.1"`. `cargo test` and `cargo clippy` are clean on rustc 1.95.0. Rule numbers refer to SKILL.md.

How it maps to the rules:

- `Ast`: rule 1. Each field is its own `Vec`, kept the same length, which is the SoA shape. `Tag` is `#[repr(u8)]` and `Data` is two `u32`s, pinned by a `const _: () = assert!(...)`.
- `InternPool`: rule 2. `hashbrown::HashTable<StrId>` stores only the 4-byte id and hashes through the byte buffer, which is what Zig's adapted context does. Std's `HashMap` can't hash a key through outside storage. Its stdlib form is a `HashMap<Box<str>, StrId>` plus a `Vec` of names, which stores each name twice.
- `class` / `info`: rule 4. A `const fn` whose `match` has no `_` arm. Adding a variant to `Tag` produced four `E0004 non-exhaustive patterns` errors, one per decision site.
- `Code`, `info`, `Diag`: rule 5. `eval` returns `Option<i64>` and appends to `&mut Vec<Diag>`, the `(T, Vec<Error>)` shape from rust-analyzer.
- `eval`'s `work` stack: rule 7. The last test walks 10⁶ levels.

The newtype ids (`StrId(u32)`, `NodeId(u32)`) are the typed indices of the `rust` skill's `types.md`. rust-analyzer's own arenas (`la-arena`'s `Idx<T>`) have the same shape.

```rust
//! A minimal frontend layout: node columns with `extra_data`, an intern pool,
//! a per-tag fact function, coded diagnostics and a worklist walk.

use hashbrown::HashTable;
use std::hash::{BuildHasher, BuildHasherDefault, DefaultHasher};

/// Dense id of an interned name, in first-seen order.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct StrId(u32);

/// Names stored once: one byte buffer plus end offsets. The table holds only
/// ids and hashes the stored bytes, so each slot is 4 bytes.
#[derive(Default)]
pub struct InternPool {
    bytes: String,
    ends: Vec<u32>,
    table: HashTable<StrId>,
    hasher: BuildHasherDefault<DefaultHasher>,
}

impl InternPool {
    pub fn intern(&mut self, s: &str) -> StrId {
        let hash = self.hasher.hash_one(s);
        let Self { bytes, ends, table, hasher } = self;
        if let Some(&id) = table.find(hash, |&id| text(bytes, ends, id) == s) {
            return id;
        }
        let id = StrId(u32::try_from(ends.len()).expect("more than u32::MAX names"));
        bytes.push_str(s);
        ends.push(u32::try_from(bytes.len()).expect("more than u32::MAX bytes"));
        table.insert_unique(hash, id, |&id| hasher.hash_one(text(bytes, ends, id)));
        id
    }

    pub fn get(&self, id: StrId) -> &str {
        text(&self.bytes, &self.ends, id)
    }
}

fn text<'a>(bytes: &'a str, ends: &[u32], id: StrId) -> &'a str {
    let i = id.0 as usize;
    let start = if i == 0 { 0 } else { ends[i - 1] as usize };
    &bytes[start..ends[i] as usize]
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeId(u32);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub enum Tag {
    /// `lhs` = `StrId`.
    Identifier,
    /// `lhs` = the value.
    IntLiteral,
    /// `lhs`, `rhs` = operand `NodeId`.
    Add,
    Mul,
    /// `lhs` = callee `StrId`, `rhs` = offset in `extra_data` of `[len, arg0, ...]`.
    Call,
}

#[derive(Clone, Copy)]
pub struct Data {
    pub lhs: u32,
    pub rhs: u32,
}

const _: () = assert!(size_of::<Tag>() == 1 && size_of::<Data>() == 8);

/// What a walk needs to know about a tag. The `match` has no `_` arm, so a
/// new tag does not compile until its row exists.
#[derive(Clone, Copy)]
enum Class {
    Leaf,
    Binary,
    Call,
}

const fn class(t: Tag) -> Class {
    match t {
        Tag::Identifier | Tag::IntLiteral => Class::Leaf,
        Tag::Add | Tag::Mul => Class::Binary,
        Tag::Call => Class::Call,
    }
}

/// Stable codes: never renumbered or reused.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Code {
    E0201,
    E0410,
}

pub struct Info {
    pub title: &'static str,
    pub clause: &'static str,
}

pub const fn info(c: Code) -> Info {
    match c {
        Code::E0201 => Info { title: "expected an expression", clause: "A.8.3" },
        Code::E0410 => Info { title: "function call is parsed but not yet evaluated", clause: "4.7" },
    }
}

/// One diagnostic row; text is rendered later from `info` and the token.
#[derive(Debug)]
pub struct Diag {
    pub code: Code,
    pub token: u32,
}

/// Node columns, one `Vec` per field, kept the same length.
#[derive(Default)]
pub struct Ast {
    tags: Vec<Tag>,
    main_tokens: Vec<u32>,
    datas: Vec<Data>,
    extra_data: Vec<u32>,
}

impl Ast {
    pub fn add(&mut self, tag: Tag, main_token: u32, lhs: u32, rhs: u32) -> NodeId {
        let id = NodeId(u32::try_from(self.tags.len()).expect("more than u32::MAX nodes"));
        self.tags.push(tag);
        self.main_tokens.push(main_token);
        self.datas.push(Data { lhs, rhs });
        id
    }

    /// Appends `[len, items...]` and returns the offset of `len`.
    pub fn add_list(&mut self, items: &[NodeId]) -> u32 {
        let at = self.extra_data.len() as u32;
        self.extra_data.push(items.len() as u32);
        self.extra_data.extend(items.iter().map(|n| n.0));
        at
    }
}

/// Post-order evaluation on an explicit stack. A call is refused with a coded
/// diagnostic rather than evaluated to a guess.
pub fn eval(ast: &Ast, root: NodeId, env: &[i64], diags: &mut Vec<Diag>) -> Option<i64> {
    let mut work = vec![(root, false)];
    let mut values: Vec<i64> = Vec::new();
    while let Some((node, operands_done)) = work.pop() {
        let i = node.0 as usize;
        let Data { lhs, rhs } = ast.datas[i];
        match class(ast.tags[i]) {
            Class::Leaf => values.push(match ast.tags[i] {
                Tag::Identifier => env[lhs as usize],
                Tag::IntLiteral => lhs.into(),
                Tag::Add | Tag::Mul | Tag::Call => unreachable!("class says leaf"),
            }),
            Class::Binary if !operands_done => {
                // Pushed in reverse so lhs is evaluated first.
                work.extend([(node, true), (NodeId(rhs), false), (NodeId(lhs), false)]);
            }
            Class::Binary => {
                let r = values.pop()?;
                let l = values.pop()?;
                values.push(match ast.tags[i] {
                    Tag::Add => l.wrapping_add(r),
                    Tag::Mul => l.wrapping_mul(r),
                    Tag::Identifier | Tag::IntLiteral | Tag::Call => unreachable!("class says binary"),
                });
            }
            Class::Call => {
                diags.push(Diag { code: Code::E0410, token: ast.main_tokens[i] });
                return None;
            }
        }
    }
    debug_assert_eq!(values.len(), 1);
    values.pop()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn intern_returns_one_id_per_text() {
        let mut pool = InternPool::default();
        let a = pool.intern("alpha");
        let b = pool.intern("beta");
        assert_eq!(a, pool.intern("alpha"));
        assert_ne!(a, b);
        assert_eq!(pool.get(b), "beta");
    }

    #[test]
    fn eval_folds_and_refuses_a_call() {
        let mut pool = InternPool::default();
        let mut ast = Ast::default();
        let mut diags = Vec::new();
        // x * 3 + 4, with x = 5
        let x = pool.intern("x");
        let n_x = ast.add(Tag::Identifier, 0, x.0, 0);
        let n_3 = ast.add(Tag::IntLiteral, 2, 3, 0);
        let n_mul = ast.add(Tag::Mul, 1, n_x.0, n_3.0);
        let n_4 = ast.add(Tag::IntLiteral, 4, 4, 0);
        let n_add = ast.add(Tag::Add, 3, n_mul.0, n_4.0);
        assert_eq!(eval(&ast, n_add, &[5], &mut diags), Some(19));

        let f = pool.intern("f");
        let args = ast.add_list(&[n_x, n_3]);
        let n_call = ast.add(Tag::Call, 6, f.0, args);
        assert_eq!(eval(&ast, n_call, &[5, 0], &mut diags), None);
        assert_eq!(diags[0].code, Code::E0410);
        assert_eq!(info(diags[0].code).clause, "4.7");
        assert_eq!(ast.extra_data[args as usize], 2);
    }

    #[test]
    fn a_million_levels_walk_on_the_heap() {
        let mut ast = Ast::default();
        let mut top = ast.add(Tag::IntLiteral, 0, 1, 0);
        for _ in 0..1_000_000 {
            let one = ast.add(Tag::IntLiteral, 0, 1, 0);
            top = ast.add(Tag::Add, 0, one.0, top.0);
        }
        assert_eq!(eval(&ast, top, &[], &mut Vec::new()), Some(1_000_001));
    }
}
```

## Where the real ones are

- rust-analyzer `crates/parser`: hand-written recursive descent that emits a flat event stream (`start node`, `finish node`). It doesn't depend on the tree or token types, and it never fails: it returns `(T, Vec<Error>)`.
- rust-analyzer `crates/syntax`: rowan trees, a value type built per file that carries no semantic info. Its parser tests are data-driven `.rs` → `.txt` pairs regenerated with `UPDATE_EXPECT=1`, the Rust form of rule 9's goldens.
