//! The deck's run plan read before its statements are lowered: what HSPICE runs around every
//! analysis or one (a `.temp` list, the `.alter` blocks, a swept `.data` table) must be known
//! where the first analysis stands, and every statement of it is framing there
//! (`read_run::deck_loop`, `read_run::rows`).
//!
//! Cites: `rewrite/DIALECT-HSPICE.md` §6 (branch `differences`); settle `temp_list.sp`,
//! `alter.sp`, `data_sweep.sp` (ESPice 0.3.0 in HSPICE mode: ESPice is not HSPICE).
//!
//! Contents: `pub(super) struct AlterPlan`, `pub(super) fn first_alter(&Ast, &Tokens) -> usize`,
//! `pub(super) fn alter_plan(&Ast, &Tokens) -> Option<AlterPlan>`,
//! `pub(super) fn temp_list(&Ast, &Tokens) -> Option<Node>`,
//! `pub(super) fn data_tables(&Ast, &Tokens) -> Vec<(u32, String, Vec<String>, Vec<String>)>`

use super::cst::{extent, rest};
use super::name;
use crate::syntax::ast::{Ast, Node, Tag};
use crate::syntax::token::Tokens;

/// A deck's `.alter` blocks read as one table: after its analyses run, each block reruns every
/// analysis with its `.param` changes, which hold into the later blocks (settle `alter.sp`:
/// 1k, then 2k; ESPice only).
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AlterPlan {
    /// The root statements from the first `.alter` on (each is framing: the loop states them).
    pub(super) nodes: Vec<u32>,
    /// The parameters any block sets, in first-set order.
    pub(super) columns: Vec<String>,
    /// One row per pass, the deck's own first: each column's value text.
    pub(super) rows: Vec<Vec<String>>,
}

/// The index among the root statements of the first `.alter` card (their count when none).
pub(super) fn first_alter(ast: &Ast<'_>, toks: &Tokens<'_>) -> usize {
    let root = ast.fields(ast.root());
    root.iter()
        .position(|&s| is_alter(ast, toks, Node(s)))
        .unwrap_or(root.len())
}

/// The `.alter` blocks of a deck as one table, or `None` when it has none or one this reader
/// does not state: a block holding anything but `.param` cards, an `.alter` with a title, no
/// top-level analysis before the first `.alter`, a parameter with no top-level `.param` before
/// it. ponytail: `.param` changes only; element, model and option changes wait for a run of
/// each on ESPice.
pub(super) fn alter_plan(ast: &Ast<'_>, toks: &Tokens<'_>) -> Option<AlterPlan> {
    let root = ast.fields(ast.root());
    let first = first_alter(ast, toks);
    if first == root.len()
        || !root[..first]
            .iter()
            .any(|&s| ast.tag(Node(s)) == Tag::Analysis)
    {
        return None;
    }
    let mut base: Vec<(String, String)> = Vec::new();
    for &s in &root[..first] {
        if ast.tag(Node(s)) == Tag::Param {
            base.extend(super::declare::assigns(ast, toks, Node(s)));
        }
    }
    let mut plan = AlterPlan {
        nodes: root[first..].to_vec(),
        columns: Vec::new(),
        rows: Vec::new(),
    };
    let mut blocks: Vec<Vec<(String, String)>> = Vec::new();
    for &s in &root[first..] {
        let s = Node(s);
        match ast.tag(s) {
            _ if is_alter(ast, toks, s) => {
                if !rest(toks, ast, s).trim().is_empty() {
                    return None;
                }
                blocks.push(Vec::new());
            }
            Tag::Param => {
                let set = super::declare::assigns(ast, toks, s);
                for (k, _) in &set {
                    if !plan.columns.iter().any(|c| name::key(c) == name::key(k)) {
                        plan.columns.push(k.clone());
                    }
                }
                blocks
                    .last_mut()
                    .expect("a block after an .alter")
                    .extend(set);
            }
            Tag::End => {}
            _ => return None,
        }
    }
    let last = |set: &[(String, String)], c: &str| {
        set.iter()
            .rev()
            .find(|(k, _)| name::key(k) == name::key(c))
            .map(|(_, v)| v.clone())
    };
    if plan.columns.is_empty() {
        return None;
    }
    let mut row: Vec<String> = Vec::new();
    for c in &plan.columns {
        row.push(last(&base, c)?);
    }
    plan.rows.push(row.clone());
    for block in &blocks {
        for (k, c) in plan.columns.iter().enumerate() {
            if let Some(v) = last(block, c) {
                row[k] = v;
            }
        }
        plan.rows.push(row.clone());
    }
    Some(plan)
}

/// The deck's one top-level `.temp` card when it lists more than one value, and no other
/// `.temp` card stands at the top level.
pub(super) fn temp_list(ast: &Ast<'_>, toks: &Tokens<'_>) -> Option<Node> {
    let mut cards = ast
        .fields(ast.root())
        .iter()
        .map(|&n| Node(n))
        .filter(|&n| ast.tag(n) == Tag::Temp);
    let card = cards.next()?;
    let words = rest(toks, ast, card);
    let list = super::read_run::split_words(words.trim().trim_start_matches('=')).len() > 1;
    (list && cards.next().is_none()).then_some(card)
}

/// The top-level `.data name col… values… .enddata` tables a top-level `sweep data=name` reads,
/// whose values fill whole rows: (node, name, columns, values row-major). ponytail: inline
/// tables only (no `merge`, `laminated`, files).
pub(super) fn data_tables(
    ast: &Ast<'_>,
    toks: &Tokens<'_>,
) -> Vec<(u32, String, Vec<String>, Vec<String>)> {
    let root = ast.fields(ast.root());
    let text = |n: Node| {
        let (a, b) = extent(ast, toks, n);
        toks.source()[toks.start(a) as usize..toks.end(b) as usize].to_ascii_lowercase()
    };
    let used: Vec<String> = root
        .iter()
        .map(|&n| Node(n))
        .filter(|&n| ast.tag(n) == Tag::Analysis)
        .map(|n| text(n).split_whitespace().collect::<String>())
        .collect();
    let mut out = Vec::new();
    for &n in root.iter().filter(|&&n| ast.tag(Node(n)) == Tag::Data) {
        let block = text(Node(n));
        let mut lines = block.lines();
        let head: Vec<&str> = lines.next().unwrap_or("").split_whitespace().collect();
        let [kw, name, columns @ ..] = head.as_slice() else {
            continue;
        };
        let values: Vec<String> = lines
            .take_while(|l| !l.trim_start().starts_with(".enddata"))
            .flat_map(|l| l.split_whitespace().map(str::to_string))
            .collect();
        let whole =
            !columns.is_empty() && !values.is_empty() && values.len().is_multiple_of(columns.len());
        let swept = used
            .iter()
            .any(|u| u.contains(&format!("sweepdata={name}")));
        if *kw == ".data" && whole && swept {
            let columns = columns.iter().map(|c| c.to_string()).collect();
            out.push((n, name.to_string(), columns, values));
        }
    }
    out
}

// ---- private ----

/// Whether `s` is an `.alter` card.
fn is_alter(ast: &Ast<'_>, toks: &Tokens<'_>, s: Node) -> bool {
    toks.text(ast.main_token(s)).eq_ignore_ascii_case(".alter")
}
