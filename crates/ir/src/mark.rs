//! The builder's undo (the dialects' audit, row 5): a `Mark` records how long every table, pool
//! and index of a design is; `Builder::truncate` drops everything appended since, so a reader
//! can push while it validates a statement and take a refused statement's rows back instead of
//! building a second tree first. Design in, the same design as it stood at the mark out.
//!
//! What it undoes is appends only: a builder call that changes a row which existed at the mark
//! (`Builder::resolve`, `Builder::bind_ref` on an older expression or item) is not undone, and
//! the caller does not make one between a mark and its truncate. Every table is listed through
//! an exhaustive destructuring of its struct (`Design`, `CircuitDetails`, `RunDetails`), so a
//! new table that is not listed is a compile error, not a silent leftover.
//!
//! Cites: `rewrite/audit/2026-11-12-dialects.md` row 5 (on the `rulings` branch);
//! `src/syntax/ast.rs` `Ast::mark`/`Ast::reset` (the root crate's precedent).
//!
//! Contents: `pub struct Mark`, `Builder::mark(&mut self) -> Mark`, `Builder::truncate(&mut self,
//! Mark)`; crate: `pub(crate) trait Column` (`Column::count`, `Column::cut`)

use crate::build::Builder;
use crate::design::{Design, ItemRow};
use crate::id::{DefId, ItemRef, NameId};
use crate::item::ItemKind;
use crate::names::{Names, Texts};
use crate::table::Table;

/// The lengths of a design's tables at one moment (`Builder::mark`). Valid for the design it
/// was taken from, until a truncate to an earlier mark.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    /// Every column's length, in `columns` order.
    lens: Vec<u32>,
    /// Names interned, texts pushed, definitions made, scopes opened.
    names: u32,
    texts: u32,
    defs: u32,
    scopes: u32,
    /// Rows of each item table, by `ItemKind` (the scope lists keep the items below these).
    items: [u32; ItemKind::ALL.len()],
}

/// A table the undo cuts back by length. Implemented for the plain columns (`Vec`) and the item
/// tables (`Table`, rows and spans together).
pub(crate) trait Column {
    /// Its length.
    fn count(&self) -> u32;
    /// Drops every row from `len` on. Asserts `len <= count()`.
    fn cut(&mut self, len: u32);
}

impl Builder<'_> {
    /// Where the design stands: pass it to [`Builder::truncate`] to drop everything appended
    /// after. O(tables), allocates one small vector. Takes `&mut self` only to list the
    /// tables once (the same walk the truncate cuts).
    ///
    /// ```
    /// use spicerack_ir::{Design, Role, Span};
    /// let mut d = Design::new();
    /// let mut b = d.builder();
    /// let (_, top) = b.file("t.sp", Role::Deck);
    /// let mark = b.mark();
    /// let v = b.num(1.0); // a reader pushes while it validates ...
    /// b.param(top, "p", v, Span::NONE);
    /// b.truncate(mark); // ... and takes a refused statement back
    /// assert!(d.items(d.file(d.entry()).top).is_empty());
    /// assert!(d.find_name("p").is_none());
    /// ```
    pub fn mark(&mut self) -> Mark {
        let lens: Vec<u32> = columns_of(self.design).iter().map(|c| c.count()).collect();
        let d: &Design = self.design;
        let mut items = [0u32; ItemKind::ALL.len()];
        for kind in ItemKind::ALL {
            items[kind as usize] = for_kind!(kind, R => R::table(d).len()) as u32;
        }
        Mark {
            lens,
            names: d.names.len() as u32,
            texts: d.texts.len() as u32,
            defs: d.defs.len() as u32,
            scopes: d.scopes.len() as u32,
            items,
        }
    }

    /// Drops every row, name, text, definition, scope and list entry appended since `mark`, so
    /// the design is as it stood then, its next ids the same. Asserts `mark` is no later than
    /// the design (a mark of this design, not truncated past). Rows changed in place since the
    /// mark stay changed (module header). O(what is dropped + scopes).
    pub fn truncate(&mut self, mark: Mark) {
        let d = &mut *self.design;
        assert!(
            mark.names as usize <= d.names.len(),
            "a mark of this design"
        );
        assert!(mark.defs as usize <= d.defs.len(), "a mark of this design");
        d.derived_changed();
        d.element_names.take();
        // the index entries of the dropped definitions
        for k in mark.defs as usize..d.defs.len() {
            let def = d.defs[k];
            let key = (def.scope, def.kind, def.name);
            if d.def_index.get(&key) == Some(&DefId(k as u32)) {
                d.def_index.remove(&key);
            }
        }
        d.defs.truncate(mark.defs as usize);
        // the dropped items, from the end of the lists that survive (pushes append)
        let items = mark.items;
        let new = |i: &ItemRef| i.index >= items[i.kind as usize];
        d.scope_items.truncate(mark.scopes as usize);
        for list in &mut d.scope_items {
            let keep = list.iter().rposition(|i| !new(i)).map_or(0, |k| k + 1);
            assert!(
                list[keep..].iter().all(new),
                "pushes append to a scope's list"
            );
            list.truncate(keep);
        }
        d.scopes.truncate(mark.scopes as usize);
        cut_names(&mut d.names, mark.names);
        cut_texts(&mut d.texts, mark.texts);
        let mut columns = columns_of(d);
        assert!(columns.len() == mark.lens.len(), "one length per column");
        for (column, &len) in columns.iter_mut().zip(&mark.lens) {
            column.cut(len);
        }
        assert!(d.scopes.len() == d.scope_items.len(), "one list per scope");
    }
}

impl<T> Column for Vec<T> {
    fn count(&self) -> u32 {
        self.len() as u32
    }

    fn cut(&mut self, len: u32) {
        assert!(len as usize <= self.len(), "a mark no later than the table");
        self.truncate(len as usize);
    }
}

impl<R> Column for Table<R> {
    fn count(&self) -> u32 {
        self.rows.len() as u32
    }

    fn cut(&mut self, len: u32) {
        assert!(
            len as usize <= self.rows.len(),
            "a mark no later than the table"
        );
        self.rows.truncate(len as usize);
        self.spans.truncate(len as usize);
    }
}

// ---- private ----

/// Every plain column of `d`: the kernel's, every item table, the circuit and run details. The
/// destructuring lists every field of `Design`, so a new one is a compile error here.
fn columns_of(d: &mut Design) -> Vec<&mut dyn Column> {
    let Design {
        names: _,
        texts: _,
        files,
        scopes: _,
        scope_items: _,
        defs: _,
        def_index: _,
        exprs,
        name_pool,
        expr_pool,
        param_pool,
        branch_pool,
        titles,
        comments,
        includes,
        globals,
        loads,
        opaques,
        subckts,
        sections,
        ifs,
        params,
        funcs,
        models,
        device_libraries,
        statistics,
        elements,
        analyses,
        loops,
        alters,
        options,
        envs,
        node_sets,
        saves,
        prints,
        measures,
        elaborates,
        resets,
        circuit,
        run,
        classes: _,
        forms: _,
        element_names: _,
        #[cfg(any(test, feature = "gen"))]
            class_computes: _,
    } = d;
    let mut out: Vec<&mut dyn Column> = vec![
        files,
        &mut exprs.tags,
        &mut exprs.subs,
        &mut exprs.a,
        &mut exprs.b,
        &mut exprs.calls,
        &mut exprs.nums,
        &mut exprs.long,
        name_pool,
        expr_pool,
        param_pool,
        branch_pool,
    ];
    out.extend([
        titles as &mut dyn Column,
        comments,
        includes,
        globals,
        loads,
        opaques,
        subckts,
        sections,
        ifs,
        params,
        funcs,
        models,
        device_libraries,
        statistics,
        elements,
        analyses,
        loops,
        alters,
        options,
        envs,
        node_sets,
        saves,
        prints,
        measures,
        elaborates,
        resets,
    ]);
    out.extend(circuit_columns(circuit));
    out.extend(run_columns(run));
    out
}

/// Every table of the circuit owner's details, listed by an exhaustive destructuring.
fn circuit_columns(t: &mut crate::element::CircuitDetails) -> Vec<&mut dyn Column> {
    let crate::element::CircuitDetails {
        passives,
        parasitics,
        couplings,
        sources,
        ports,
        distofs,
        controlled,
        polys,
        tables,
        behavs,
        laplaces,
        freq_tables,
        delays,
        semis,
        switches,
        lines,
        pulses,
        sins,
        exps,
        pwls,
        sffms,
        ams,
        trnoises,
        trrandoms,
        patterns,
    } = t;
    vec![
        passives as &mut dyn Column,
        parasitics,
        couplings,
        sources,
        ports,
        distofs,
        controlled,
        polys,
        tables,
        behavs,
        laplaces,
        freq_tables,
        delays,
        semis,
        switches,
        lines,
        pulses,
        sins,
        exps,
        pwls,
        sffms,
        ams,
        trnoises,
        trrandoms,
        patterns,
    ]
}

/// Every table of the run owner's details, listed by an exhaustive destructuring.
fn run_columns(t: &mut crate::analysis::RunDetails) -> Vec<&mut dyn Column> {
    let crate::analysis::RunDetails {
        dcs,
        acs,
        trans,
        noises,
        tfs,
        dc_xfs,
        xfs,
        senses,
        sens_acs,
        pzs,
        sps,
        nets,
        fours,
        distos,
        hbs,
        psses,
        pacs,
        pnoises,
        pxfs,
        pstbs,
        stbs,
        timed_ops,
        sweep_loops,
        table_loops,
        monte_carlos,
        trig_targs,
        whens,
        finds,
        derivs,
        stats,
        measure_params,
        dc_axes,
        probes,
        targets,
        varies,
        correlations,
        refs,
    } = t;
    vec![
        dcs as &mut dyn Column,
        acs,
        trans,
        noises,
        tfs,
        dc_xfs,
        xfs,
        senses,
        sens_acs,
        pzs,
        sps,
        nets,
        fours,
        distos,
        hbs,
        psses,
        pacs,
        pnoises,
        pxfs,
        pstbs,
        stbs,
        timed_ops,
        sweep_loops,
        table_loops,
        monte_carlos,
        trig_targs,
        whens,
        finds,
        derivs,
        stats,
        measure_params,
        dc_axes,
        probes,
        targets,
        varies,
        correlations,
        refs,
    ]
}

/// Drops the names from `len` on, newest first, each out of its hash chain (an interned name
/// heads its bucket until a newer one of the same hash arrives, so newest-first restores every
/// bucket).
fn cut_names(names: &mut Names, len: u32) {
    let len = len as usize;
    assert!(len >= 1, "ground stays");
    for i in (len..names.len()).rev() {
        let key = crate::names::hash(names.str(NameId(i as u32)));
        let older = names.next[i];
        assert!(
            names.buckets.get(&key) == Some(&NameId(i as u32)),
            "the newest heads its chain"
        );
        if older.is_none() {
            names.buckets.remove(&key);
        } else {
            names.buckets.insert(key, older);
        }
    }
    let bytes = if len == 0 {
        0
    } else {
        names.ends[len - 1] as usize
    };
    names.ends.truncate(len);
    names.next.truncate(len);
    names.bytes.truncate(bytes);
}

/// Drops the texts from `len` on.
fn cut_texts(texts: &mut Texts, len: u32) {
    let len = len as usize;
    assert!(len <= texts.len(), "a mark no later than the texts");
    let bytes = if len == 0 {
        0
    } else {
        texts.ends[len - 1] as usize
    };
    texts.ends.truncate(len);
    texts.bytes.truncate(bytes);
}
