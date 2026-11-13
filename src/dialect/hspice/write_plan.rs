//! Loops → HSPICE's run-plan cards: a sweep tail (`.dc … sweep p a b step`), a deck's `.temp`
//! list, its `.alter` blocks, a `.data` table swept by one card (`sweep data=name`). Each form
//! is exactly what `read_run` and `read_plan` read back; [`spells`] says whether a loop is one,
//! and `write_run::admits` refuses any other (`W4303`).
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §6 (branch `differences`); settle `dc_sweep_param.sp`,
//! `temp_list.sp`, `alter.sp`, `data_sweep.sp` (ESPice 0.3.0 in HSPICE mode: ESPice is not HSPICE).
//!
//! Contents: `pub(super) fn loop_(&mut Out, ir::ItemRef)`, `pub(super) fn spells(&ir::Design,
//! ir::ItemRef) -> bool`

use super::expr_write;
use super::write::Out;
use super::write_run;
use crate::ir::{self, AnalysisKind};

/// Whether `item`, a loop, is one of HSPICE's run-plan forms.
pub(super) fn spells(design: &ir::Design, item: ir::ItemRef) -> bool {
    swept_card(design, item).is_some()
        || temp_list(design, item)
        || alter_blocks(design, item).is_some()
        || data_table(design, item).is_some()
}

/// A `Loop` in its form: a `.data` table and its card, `.alter` blocks before `.end`, a `.temp`
/// list, or a card with a `sweep param values` tail; `admits` refused every other loop.
pub(super) fn loop_(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    if let Some((table, one)) = data_table(d, item) {
        // the table, then the card with `sweep data=name` (settle data_sweep.sp)
        let l: ir::Loop = d.row(item.index);
        w.line(&table);
        let t = format!("{} sweep data={}", write_run::card(d, one), d.str(l.name));
        return w.line(&t);
    }
    if let Some(text) = alter_blocks(d, item) {
        // the analyses where the loop stands, the blocks before `.end` (settle alter.sp)
        let l: ir::Loop = d.row(item.index);
        w.before_end(&text);
        return w.open(l.body, String::new());
    }
    if temp_list(d, item) {
        // `.temp v1 v2 …` where the loop stands, then its analyses (settle temp_list.sp)
        let l: ir::Loop = d.row(item.index);
        let s: ir::plan::SweepLoop = d.detail(l.detail);
        let mut t = String::from(".temp");
        for &e in d.exprs(s.sweep.values) {
            write_run::value(d, e, &mut t);
        }
        w.line(&t);
        return w.open(l.body, String::new());
    }
    let (body, s) = swept_card(d, item).expect("HSPICE write: an admitted loop");
    let mut t = write_run::card(d, body);
    t.push_str(" sweep ");
    t.push_str(d.str(s.target.a.name));
    write_run::axis(d, s.sweep, &mut t);
    w.line(&t);
}

// ---- private ----

/// The one analysis card a sweep loop wraps and its sweep, when HSPICE's tail spells it: a
/// sweep of a parameter, restored after, unnamed, around one `.dc`, `.ac` or `.tran`.
fn swept_card(d: &ir::Design, item: ir::ItemRef) -> Option<(ir::ItemRef, ir::plan::SweepLoop)> {
    let l: ir::Loop = d.row(item.index);
    if l.kind != ir::LoopKind::Sweep || l.after != ir::LoopEnd::Restore || !l.name.is_none() {
        return None;
    }
    let s: ir::plan::SweepLoop = d.detail(l.detail);
    let body = d.items(l.body);
    let one = match body {
        [one] if one.kind == ir::ItemKind::Analysis => *one,
        _ => return None,
    };
    let a: ir::Analysis = d.row(one.index);
    let kind_ok = matches!(
        a.kind,
        AnalysisKind::Dc | AnalysisKind::Ac | AnalysisKind::Tran
    );
    let spelt = [s.sweep.start, s.sweep.stop, s.sweep.step, s.sweep.points]
        .into_iter()
        .chain(d.exprs(s.sweep.values).iter().copied())
        .all(|v| v.is_none() || expr_write::spells(d, v));
    (kind_ok && spelt && s.target.kind == ir::TargetKind::Param && s.target.path.len == 0)
        .then_some((one, s))
}

/// Whether a loop is a deck's `.temp` list as the reader reads one back: an unnamed sweep of
/// the temperature over listed values, restored after, holding analyses only, standing in a
/// deck's top scope, the only loop there and with every analysis of that scope in its body.
fn temp_list(d: &ir::Design, item: ir::ItemRef) -> bool {
    let l: ir::Loop = d.row(item.index);
    if l.kind != ir::LoopKind::Sweep || l.after != ir::LoopEnd::Restore || !l.name.is_none() {
        return false;
    }
    let s: ir::plan::SweepLoop = d.detail(l.detail);
    let shape = s.target.kind == ir::TargetKind::Temp
        && s.sweep.kind == ir::SweepKind::List
        && d.exprs(s.sweep.values)
            .iter()
            .all(|&v| expr_write::spells(d, v));
    shape && deck_top(d, item).is_some()
}

/// The deck's top scope when `item`, a loop of analyses only, stands there as the only loop
/// and holds every analysis of that scope (as the reader's deck loop reads back).
fn deck_top(d: &ir::Design, item: ir::ItemRef) -> Option<ir::ScopeId> {
    let l: ir::Loop = d.row(item.index);
    if !d
        .items(l.body)
        .iter()
        .all(|i| i.kind == ir::ItemKind::Analysis)
    {
        return None;
    }
    let top = d
        .files()
        .map(|f| d.file(f))
        .find(|f| f.role == ir::Role::Deck && d.items(f.top).contains(&item))?
        .top;
    let alone = d
        .items(top)
        .iter()
        .all(|i| *i == item || !matches!(i.kind, ir::ItemKind::Loop | ir::ItemKind::Analysis));
    alone.then_some(top)
}

/// A `.data` table's text and the one analysis card its loop wraps, when `item` is the table
/// loop a `sweep data=name` reads back: named, restored after, over parameters, around one
/// `.dc`, `.ac` or `.tran`.
fn data_table(d: &ir::Design, item: ir::ItemRef) -> Option<(String, ir::ItemRef)> {
    let l: ir::Loop = d.row(item.index);
    if l.kind != ir::LoopKind::Table || l.after != ir::LoopEnd::Restore || l.name.is_none() {
        return None;
    }
    let one = match d.items(l.body) {
        [one] if one.kind == ir::ItemKind::Analysis => *one,
        _ => return None,
    };
    let a: ir::Analysis = d.row(one.index);
    if !matches!(
        a.kind,
        AnalysisKind::Dc | AnalysisKind::Ac | AnalysisKind::Tran
    ) {
        return None;
    }
    let t: ir::plan::TableLoop = d.detail(l.detail);
    let columns = d.targets(t.columns);
    let plain = |c: &ir::Target| c.kind == ir::TargetKind::Param && c.path.len == 0;
    let spelt = d.exprs(t.values).iter().all(|&v| expr_write::spells(d, v));
    if columns.is_empty() || !columns.iter().all(plain) || !spelt {
        return None;
    }
    let mut text = format!(".data {}", d.str(l.name));
    for c in columns {
        text.push(' ');
        text.push_str(d.str(c.a.name));
    }
    for row in d.exprs(t.values).chunks(columns.len()) {
        text.push('\n');
        for (k, &v) in row.iter().enumerate() {
            if k > 0 {
                text.push(' ');
            }
            expr_write::value(d, v, &mut text);
        }
    }
    text.push_str("\n.enddata");
    Some((text, one))
}

/// A deck's `.alter` blocks (`.alter`, then `.param` of every column, per row after the first)
/// when `item` is the table the reader reads them as: unnamed, restored after, a deck loop
/// ([`deck_top`]) over parameters whose first row spells as the deck's own `.param` values.
fn alter_blocks(d: &ir::Design, item: ir::ItemRef) -> Option<String> {
    let l: ir::Loop = d.row(item.index);
    if l.kind != ir::LoopKind::Table || l.after != ir::LoopEnd::Restore || !l.name.is_none() {
        return None;
    }
    let top = deck_top(d, item)?;
    let t: ir::plan::TableLoop = d.detail(l.detail);
    let columns = d.targets(t.columns);
    let values = d.exprs(t.values);
    let n = columns.len();
    let plain = |c: &ir::Target| c.kind == ir::TargetKind::Param && c.path.len == 0;
    let spelt = values.iter().all(|&v| expr_write::spells(d, v));
    if n == 0 || values.len() < 2 * n || !columns.iter().all(plain) || !spelt {
        return None;
    }
    let spell = |e: ir::ExprId| {
        let mut s = String::new();
        expr_write::value(d, e, &mut s);
        s
    };
    // the deck's own value of each column: its last `.param` before the loop
    let at = d.items(top).iter().position(|&i| i == item)?;
    for (k, c) in columns.iter().enumerate() {
        let own = d.items(top)[..at].iter().rev().find_map(|i| {
            let p: ir::item::Param = (i.kind == ir::ItemKind::Param).then(|| d.row(i.index))?;
            (p.name == c.a.name).then_some(p.value)
        })?;
        if spell(own) != spell(values[k]) {
            return None;
        }
    }
    let mut text = String::new();
    for row in values.chunks(n).skip(1) {
        text.push_str(".alter\n.param");
        for (c, &v) in columns.iter().zip(row) {
            text.push_str(&format!(" {}={}", d.str(c.a.name), spell(v)));
        }
        text.push('\n');
    }
    Some(text)
}
