//! Step `drop_unused` (R80): a definition nothing uses is not part of what a deck asks a
//! simulator to run, so one that holds a refused item (a feature the target lacks, or a
//! refusal of the target writer's `admits`) is dropped, with one `UNUSED_DROPPED` note per
//! (definition, refusal) naming the refusal. Never a used one; never
//! in a kit conversion (`Roots::Every`). Not a
//! registry row: a row that "removes" a feature only where its definition is unused would make
//! `passes::legal` call the feature removable everywhere. The spine runs it after the rules'
//! fixed point, on what is still refused, and runs the fixed point again while it drops
//! something (each such round removes a definition: bounded by their count).
//!
//! **Definitions**: subcircuits, models, parameters, functions (with the formals and the body a
//! subcircuit's item carries), and device libraries (a library row is used exactly when a used
//! model is of its family). **Roots**: under `Roots::Every` (a kit) every definition, so
//! nothing is dropped; under `Roots::Entry` every item no definition holds, in every file and
//! section, whether an include names it or not, and every definition that holds an `Env`
//! (R102: it states the run's temperature): such an item stays, so what it binds must too
//! (a library's corners are sections of definitions and includes, and an include binds no
//! definition). **Edges**: from a used item, every reference its row holds and every parameter
//! or function an expression of it reads, to that definition's item; a used item uses its
//! whole body; a model reference uses every bin whose base it names. **The safe side**: a name
//! an unresolved reference, read or call spells, or an identifier in an opaque statement's
//! text, uses every definition of that name (any case) declared at a file's top or in a section
//! (a definition inside a body is found only from inside it, and a used body uses all of it).
//!
//! What is dropped: for each refused item, the outermost unused definition
//! that encloses it (or is it), when no enclosing definition is used; then every outermost
//! unused definition whose own edges reach a dropped one (it would name a definition that is
//! gone), with the dropped one's refusals. A section that ends up empty stays.
//!
//! Cites: `rewrite/RULINGS.md` R80; `crates/ir/src/scope.rs` (`reachable`, `Scope::owner`);
//! `crates/ir/src/edit.rs` (`remove_item` leaves other rows' references as they are).
//!
//! Contents: `pub enum Roots`, `pub type Refused`,
//! `pub fn lacking(&ir::Design, &ir::FeatureSet) -> Vec<Refused>`, `pub fn dropped_for(u32) -> (ir::Code, u32)`,
//! `pub fn drop_unused(&mut ir::Design, &[Refused], Roots, &mut ir::Diags) -> u32`,
//! `pub const UNUSED_DROPPED: ir::Code`

use std::collections::{HashMap, HashSet};

use crate::ir;

/// The note at a definition dropped for being unused: `arg` packs the refusal it held
/// (`dropped_for`).
pub const UNUSED_DROPPED: ir::Code = ir::Code::new(b'U', 314);

/// What a conversion runs, and so which definitions are used whatever reads them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Roots {
    /// One deck: a definition is used when something that stays reaches it.
    Entry,
    /// A kit or a library: every definition is used.
    Every,
}

/// One refused item: the item, the refusal's code and its argument (a row of `check_legal`'s
/// `UNSUPPORTED`, or of a writer's `admits`).
pub type Refused = (ir::ItemRef, ir::Code, u32);

/// What one item's row points at: the definition items it binds, and the names (lower-cased)
/// it leaves unresolved or spells in opaque text, and the model names it references.
#[derive(Default)]
struct Edges {
    defs: Vec<ir::ItemRef>,
    names: Vec<String>,
    bases: Vec<String>,
}

// a feature refusal's argument (the feature's index) fits beside its code in a note
const _: () = assert!(ir::Feature::COUNT < 1 << 13);

/// The items `check_legal` refuses for `target`: the list `drop_unused` takes when the
/// writer's own refusals are not yet known.
pub fn lacking(design: &ir::Design, target: &ir::FeatureSet) -> Vec<Refused> {
    let mut rows = ir::Diags::new();
    ir::check_legal(design, target, &mut rows);
    let out: Vec<Refused> = rows
        .rows()
        .iter()
        .filter(|d| d.code == ir::diag::UNSUPPORTED)
        .map(|d| (d.item, d.code, d.arg))
        .collect();
    assert!(out.len() <= rows.len());
    out
}

/// The refusal an `UNUSED_DROPPED` note's `arg` names: its code and its argument.
pub fn dropped_for(arg: u32) -> (ir::Code, u32) {
    let number = (arg & 0x3fff) as u16;
    let letter = b'A' + ((arg >> 14) & 0x1f) as u8;
    assert!(letter.is_ascii_uppercase(), "a packed letter");
    (ir::Code::new(letter, number), arg >> 19)
}

/// Drops every unused definition that holds an item of `refused`, and every unused one that
/// names a dropped one; one `UNUSED_DROPPED` note per (definition, refusal). Returns how many
/// outermost definitions went (0 under `Roots::Every`).
///
/// # Panics
/// When a refusal's argument is 2^13 or more (the note packs it beside the code).
pub fn drop_unused(
    design: &mut ir::Design,
    refused: &[Refused],
    roots: Roots,
    diags: &mut ir::Diags,
) -> u32 {
    if roots == Roots::Every || refused.is_empty() {
        return 0;
    }
    let reach = design.reachable();
    let listing: HashMap<ir::ItemRef, ir::ScopeId> = reach.iter().copied().collect();
    let body = bodies(design, &reach);
    let used = used(design, &reach, &body);
    let mut drop: Vec<(ir::ItemRef, Vec<u32>)> = Vec::new();
    for &(item, code, arg) in refused {
        if let Some(def) = outermost_unused(design, &listing, &used, item) {
            add(&mut drop, def, &[pack(code, arg)]);
        }
    }
    dependents(design, &reach, &listing, &used, &body, &mut drop);
    for (def, why) in &drop {
        for &f in why {
            diags.push_at(UNUSED_DROPPED, design, *def, f);
        }
    }
    assert!(
        drop.iter().all(|(d, _)| !used.contains(d)),
        "a used definition is never dropped"
    );
    assert!(
        drop.iter().all(|(_, f)| !f.is_empty()),
        "every drop names its refusal"
    );
    let mut editor = design.editor();
    for (def, _) in &drop {
        editor.remove_item(listing[def], *def);
    }
    drop.len() as u32
}

// ---- private ----

/// Whether `kind` is a definition's item.
fn is_def(kind: ir::ItemKind) -> bool {
    matches!(
        kind,
        ir::ItemKind::Subckt
            | ir::ItemKind::Model
            | ir::ItemKind::Param
            | ir::ItemKind::Func
            | ir::ItemKind::DeviceLibrary
    )
}

/// A refusal as one note argument: the number in bits 0..14, the letter in 14..19, the
/// argument in 19..32 (`dropped_for` reads it back).
fn pack(code: ir::Code, arg: u32) -> u32 {
    // ponytail: one u32 per note row, so a refusal's argument has 13 bits (asserted below);
    // when a refusal needs more, a second argument on the IR's note row, and `dropped_for`
    // stays the only reader
    assert!(code.letter.is_ascii_uppercase(), "a code's letter");
    assert!(code.number < 1 << 14, "a code's number");
    assert!(arg < 1 << 13, "a refusal's argument fits beside its code");
    u32::from(code.number) | u32::from(code.letter - b'A') << 14 | arg << 19
}

/// Each item's body items, from the reach list.
fn bodies(
    design: &ir::Design,
    reach: &[(ir::ItemRef, ir::ScopeId)],
) -> HashMap<ir::ItemRef, Vec<ir::ItemRef>> {
    let mut body: HashMap<ir::ItemRef, Vec<ir::ItemRef>> = HashMap::new();
    for &(i, s) in reach {
        let owner = design.scope(s).owner;
        if owner != ir::ItemRef::NONE {
            body.entry(owner).or_default().push(i);
        }
    }
    body
}

/// The outermost definition `item` stands in or is, when none of that chain is used.
fn outermost_unused(
    design: &ir::Design,
    listing: &HashMap<ir::ItemRef, ir::ScopeId>,
    used: &HashSet<ir::ItemRef>,
    item: ir::ItemRef,
) -> Option<ir::ItemRef> {
    if used.contains(&item) {
        return None;
    }
    let mut found = is_def(item.kind).then_some(item);
    let mut at = *listing.get(&item)?;
    // bounded by the scope tree's depth: each step goes to a parent
    while !at.is_none() {
        let s = design.scope(at);
        if s.owner != ir::ItemRef::NONE && is_def(s.owner.kind) {
            if used.contains(&s.owner) {
                return None;
            }
            found = Some(s.owner);
        }
        at = s.parent;
    }
    found
}

/// Adds `why` (packed refusals) to `def`'s entry of `drop`, making the entry if it has none.
fn add(drop: &mut Vec<(ir::ItemRef, Vec<u32>)>, def: ir::ItemRef, why: &[u32]) {
    let at = match drop.iter().position(|d| d.0 == def) {
        Some(at) => at,
        None => {
            drop.push((def, Vec::new()));
            drop.len() - 1
        }
    };
    for &f in why {
        if !drop[at].1.contains(&f) {
            drop[at].1.push(f);
        }
    }
}

/// Every used item (the module header's analysis).
fn used(
    design: &ir::Design,
    reach: &[(ir::ItemRef, ir::ScopeId)],
    body: &HashMap<ir::ItemRef, Vec<ir::ItemRef>>,
) -> HashSet<ir::ItemRef> {
    let defs = design.definitions_all();
    let mut stack = roots(design, reach);
    let mut used = HashSet::new();
    let (mut names, mut bases, mut families) = (HashSet::new(), HashSet::new(), HashSet::new());
    // bounded: a round that does not end uses at least one more definition
    for _ in 0..=defs.len() + 1 {
        follow(design, body, &mut stack, &mut used, &mut |e, i| {
            names.extend(e.names.drain(..));
            bases.extend(e.bases.drain(..));
            if i.kind == ir::ItemKind::Model {
                families.insert(design.row::<ir::Model>(i.index).family);
            }
        });
        for &(i, _) in reach {
            let by_family = i.kind == ir::ItemKind::DeviceLibrary
                && families.contains(&design.row::<ir::DeviceLibrary>(i.index).family);
            let by_base = i.kind == ir::ItemKind::Model && {
                let base = design.row::<ir::Model>(i.index).bin_of;
                !base.is_none() && bases.contains(&design.str(base).to_ascii_lowercase())
            };
            if (by_family || by_base) && !used.contains(&i) {
                stack.push(i);
            }
        }
        for d in defs {
            let o = design.scope(d.scope).owner;
            let global = o == ir::ItemRef::NONE || o.kind == ir::ItemKind::Section;
            let named = global && names.contains(&design.str(d.name).to_ascii_lowercase());
            if named && !used.contains(&d.item) {
                stack.push(d.item);
            }
        }
        if stack.is_empty() {
            return used;
        }
    }
    unreachable!("each round uses a definition or ends")
}

/// Adds to `drop` every outermost unused definition whose own edges, or its body's, reach a
/// definition in `drop`, with that one's refusals, until none is added.
fn dependents(
    design: &ir::Design,
    reach: &[(ir::ItemRef, ir::ScopeId)],
    listing: &HashMap<ir::ItemRef, ir::ScopeId>,
    used: &HashSet<ir::ItemRef>,
    body: &HashMap<ir::ItemRef, Vec<ir::ItemRef>>,
    drop: &mut Vec<(ir::ItemRef, Vec<u32>)>,
) {
    let outer: Vec<ir::ItemRef> = reach
        .iter()
        .map(|&(i, _)| i)
        .filter(|&i| is_def(i.kind) && outermost_unused(design, listing, used, i) == Some(i))
        .collect();
    let mut reached: Vec<(ir::ItemRef, HashSet<ir::ItemRef>)> = Vec::new();
    for &u in &outer {
        let mut seen = HashSet::new();
        follow(design, body, &mut vec![u], &mut seen, &mut |_, _| {});
        reached.push((u, seen));
    }
    // bounded: each round adds a definition or ends
    for _ in 0..=outer.len() {
        let mut grew = false;
        for (u, seen) in &reached {
            if drop.iter().any(|d| d.0 == *u) {
                continue;
            }
            let hit: Vec<u32> = drop
                .iter()
                .filter(|d| seen.contains(&d.0))
                .flat_map(|d| d.1.iter().copied())
                .collect();
            if !hit.is_empty() {
                add(drop, *u, &hit);
                grew = true;
            }
        }
        if !grew {
            return;
        }
    }
    unreachable!("each round adds a definition or ends")
}

/// Marks in `seen` every item `stack`'s items reach through bound edges and bodies (drained),
/// calling `on` with each newly reached item's edges.
fn follow(
    design: &ir::Design,
    body: &HashMap<ir::ItemRef, Vec<ir::ItemRef>>,
    stack: &mut Vec<ir::ItemRef>,
    seen: &mut HashSet<ir::ItemRef>,
    on: &mut dyn FnMut(&mut Edges, ir::ItemRef),
) {
    let mut e = Edges::default();
    // bounded: each item enters `seen` once
    while let Some(i) = stack.pop() {
        if !seen.insert(i) {
            continue;
        }
        edges(design, i, &mut e);
        stack.append(&mut e.defs);
        stack.extend(body.get(&i).into_iter().flatten().copied());
        on(&mut e, i);
    }
}

/// `item`'s edges, into `out` (cleared first).
fn edges(design: &ir::Design, item: ir::ItemRef, out: &mut Edges) {
    let defs = design.definitions_all();
    out.defs.clear();
    out.names.clear();
    out.bases.clear();
    let lower = |n: ir::NameId| design.str(n).to_ascii_lowercase();
    let mut refs = Vec::new();
    design.item_refs(item, &mut refs);
    for &(r, kind) in &refs {
        if !r.name.is_none() && kind == ir::DefKind::Model {
            out.bases.push(lower(r.name));
        }
        if r.is_resolved() {
            out.defs.push(defs[r.def.index()].item);
        } else if !r.name.is_none() {
            out.names.push(lower(r.name));
        }
    }
    if item.kind == ir::ItemKind::Opaque {
        let o: ir::item::Opaque = design.row(item.index);
        words(design.text(o.text), &mut out.names);
    }
    let mut stack = Vec::new();
    expr_roots(design, item, &mut stack);
    let mut seen = HashSet::new();
    // bounded: each expression row once
    while let Some(x) = stack.pop() {
        if x.is_none() || !seen.insert(x) {
            continue;
        }
        let read = match design.expr(x) {
            ir::Expr::Param(r) => Some(r),
            ir::Expr::UserCall(r, l) => {
                stack.extend_from_slice(design.exprs(l));
                Some(r)
            }
            ir::Expr::Call(_, l) | ir::Expr::Draw(_, _, _, l) | ir::Expr::List(l) => {
                stack.extend_from_slice(design.exprs(l));
                None
            }
            ir::Expr::Unary(_, a) => {
                stack.push(a);
                None
            }
            ir::Expr::Binary(_, a, b) => {
                stack.extend([a, b]);
                None
            }
            ir::Expr::Ternary(c, a, b) => {
                stack.extend([c, a, b]);
                None
            }
            _ => None,
        };
        match read {
            Some(r) if r.is_resolved() => out.defs.push(defs[r.def.index()].item),
            Some(r) if !r.name.is_none() => out.names.push(lower(r.name)),
            _ => {}
        }
    }
}

/// The roots: the items no definition holds, but sections (a section's items are roots by
/// themselves, and a used section would use its definitions).
fn roots(design: &ir::Design, reach: &[(ir::ItemRef, ir::ScopeId)]) -> Vec<ir::ItemRef> {
    let held = |mut at: ir::ScopeId| {
        // bounded by the scope tree's depth: each step goes to a parent
        while !at.is_none() {
            let s = design.scope(at);
            if s.owner != ir::ItemRef::NONE && is_def(s.owner.kind) {
                return true;
            }
            at = s.parent;
        }
        false
    };
    let kind = |i: ir::ItemRef| !is_def(i.kind) && i.kind != ir::ItemKind::Section;
    let mut out: Vec<ir::ItemRef> = reach
        .iter()
        .filter(|&&(i, s)| kind(i) && !held(s))
        .map(|p| p.0)
        .collect();
    // R102: an `Env` states the run's temperature wherever it stands, so the definitions that
    // hold one are used (dropping it would make the design carry the default temperature)
    for &(i, mut at) in reach {
        if i.kind != ir::ItemKind::Env {
            continue;
        }
        // bounded by the scope tree's depth: each step goes to a parent
        while !at.is_none() {
            let s = design.scope(at);
            if s.owner != ir::ItemRef::NONE && is_def(s.owner.kind) {
                out.push(s.owner);
            }
            at = s.parent;
        }
    }
    out
}

/// The expression roots `item`'s row holds, into `out`.
fn expr_roots(design: &ir::Design, item: ir::ItemRef, out: &mut Vec<ir::ExprId>) {
    let mut handles = Vec::new();
    design.item_handles(item, &mut handles);
    for h in &handles {
        match *h {
            ir::Handle::Expr(e) | ir::Handle::ExprDyn(e) => out.push(e),
            ir::Handle::Exprs(l) => out.extend_from_slice(design.exprs(l)),
            ir::Handle::Params(l) => out.extend(design.params(l).iter().map(|p| p.1)),
            ir::Handle::Branches(l) => out.extend(design.branches(l).iter().map(|b| b.cond)),
            ir::Handle::Name(_)
            | ir::Handle::Ref(..)
            | ir::Handle::Names(_)
            | ir::Handle::Scope(_)
            | ir::Handle::Text(_) => {}
        }
    }
}

/// The identifiers of an opaque statement's text, lower-cased, into `out`.
fn words(text: &str, out: &mut Vec<String>) {
    let ident = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '.' || c == '$';
    for w in text.split(|c: char| !ident(c)).filter(|w| !w.is_empty()) {
        out.push(w.to_ascii_lowercase());
    }
}
