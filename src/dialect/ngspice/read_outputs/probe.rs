//! One probe word of an output card → a [`Want`] (kind, part, path segments as written), read
//! by the card it stands in ([`Ctx`]), and a `Want` → an `ir::Probe` through the builder. The
//! rules are `read_outputs.rs`'s header (ngspice-45 `dotcards.c` `gettoks`, `fixem`;
//! `com_measure2.c` `correct_vec`, `get_value`).
//!
//! Contents: `pub(in crate::dialect::ngspice) enum Ctx`, `struct Want`, `fn probe_word`,
//! `fn node_path`, `fn element`, `fn build`

use super::No;
use crate::dialect::ngspice::read::Lower;
use crate::ir::{self, AcPart, AnalysisKind, ProbeKind};

/// Which card a probe word stands in: each reads `v(…)` its own way.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::dialect::ngspice) enum Ctx {
    /// `.save`.
    Save,
    /// `.print <kind>`.
    Print(AnalysisKind),
    /// `.meas <kind>`.
    Meas(AnalysisKind),
}

/// A probe read but not yet appended: its kind, part and path segments as written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::dialect::ngspice) struct Want {
    /// The probe kind: `V`, `Vdiff`, `I` or `Param`.
    pub(in crate::dialect::ngspice) kind: ProbeKind,
    /// The part read.
    pub(in crate::dialect::ngspice) part: AcPart,
    /// The node or element path.
    pub(in crate::dialect::ngspice) a: Vec<String>,
    /// `Vdiff`: the second node path; `Param`: the quantity.
    pub(in crate::dialect::ngspice) b: Vec<String>,
}

/// The probes a word names in `ctx` (`.save v(a,b)` names two).
pub(in crate::dialect::ngspice) fn probe_word(word: &str, ctx: Ctx) -> Result<Vec<Want>, No> {
    if let Some(rest) = word.strip_prefix('@') {
        let (dev, q) = rest
            .strip_suffix(']')
            .and_then(|r| r.split_once('['))
            .ok_or(No::Untyped)?;
        if dev.is_empty() || q.is_empty() || q.contains(['[', ']']) || complex(ctx) {
            return Err(No::Untyped);
        }
        return Ok(vec![want(
            ProbeKind::Param,
            AcPart::None,
            element(dev),
            vec![q.into()],
        )]);
    }
    if word.len() > 7 && word[word.len() - 7..].eq_ignore_ascii_case("#branch") {
        let e = &word[..word.len() - 7];
        return current(e, ctx);
    }
    let (head, inner) = word
        .strip_suffix(')')
        .and_then(|w| w.split_once('('))
        .ok_or(No::Untyped)?;
    let args: Vec<&str> = inner.split(',').collect();
    let plain = |a: &&str| !a.is_empty() && !a.contains(['(', ')', '\'', '{', '}', '=']);
    if args.len() > 2 || !args.iter().all(plain) {
        return Err(No::Untyped);
    }
    let head = head.to_ascii_lowercase();
    match head.as_str() {
        "i" if args.len() == 1 => current(args[0], ctx),
        "vi" => Err(No::Invalid), // gettoks reads `vi(` as `i(`: a branch save, the run stops
        "v" | "vm" | "vp" | "vr" | "vdb" => voltage(&head, &args, ctx),
        _ => Err(No::Untyped),
    }
}

/// A node path: `x1.x2.n` is `[x1, x2, n]` when every instance segment leads with `x`; any
/// other name is one segment.
pub(in crate::dialect::ngspice) fn node_path(name: &str) -> Vec<String> {
    let segs: Vec<&str> = name.split('.').collect();
    let path = segs.len() > 1
        && segs.iter().all(|s| !s.is_empty())
        && segs[..segs.len() - 1].iter().all(|s| x_led(s));
    if path {
        segs.iter().map(|s| s.to_string()).collect()
    } else {
        vec![name.to_string()]
    }
}

/// An element path: `v.x1.v9` (letter, instances, name) is `[x1, v9]`; any other name is one
/// segment (its whole name, R23).
pub(in crate::dialect::ngspice) fn element(name: &str) -> Vec<String> {
    let segs: Vec<&str> = name.split('.').collect();
    let last = segs[segs.len() - 1];
    let path = segs.len() > 2
        && segs[0].len() == 1
        && segs.iter().all(|s| !s.is_empty())
        && segs[1..segs.len() - 1].iter().all(|s| x_led(s))
        && last.as_bytes()[0].eq_ignore_ascii_case(&segs[0].as_bytes()[0]);
    if path {
        segs[1..].iter().map(|s| s.to_string()).collect()
    } else {
        vec![name.to_string()]
    }
}

/// The probes of `want` appended through the builder (names folded; a node `gnd` is `0`).
pub(in crate::dialect::ngspice) fn build(lw: &mut Lower<'_, '_>, w: &Want) -> ir::Probe {
    let node = matches!(w.kind, ProbeKind::V | ProbeKind::Vdiff);
    let fold = |lw: &mut Lower<'_, '_>, path: &[String], node: bool| -> Vec<String> {
        let last = path.len() - 1;
        path.iter()
            .enumerate()
            .map(|(k, s)| {
                if node && k == last {
                    lw.names.node(s)
                } else {
                    lw.names.fold(s)
                }
            })
            .collect()
    };
    let a = fold(lw, &w.a, node);
    let a: Vec<&str> = a.iter().map(String::as_str).collect();
    let p = match w.kind {
        ProbeKind::V => lw.b.probe_v(&a),
        ProbeKind::Vdiff => {
            let b = fold(lw, &w.b, true);
            let b: Vec<&str> = b.iter().map(String::as_str).collect();
            lw.b.probe_vdiff(&a, &b)
        }
        ProbeKind::I => lw.b.probe_i(&a),
        _ => {
            assert_eq!(w.kind, ProbeKind::Param, "a reader probe kind");
            let q = lw.names.fold(&w.b[0]);
            lw.b.probe_param(&a, &q)
        }
    };
    p.with_part(w.part)
}

// ---- private ----

/// `i(e)` or `e#branch`: in an ac or sp measure its real part (`get_value`).
fn current(e: &str, ctx: Ctx) -> Result<Vec<Want>, No> {
    if e.is_empty() || e.contains(['(', ')', ',']) {
        return Err(No::Untyped);
    }
    let part = if complex(ctx) {
        AcPart::Re
    } else {
        AcPart::None
    };
    Ok(vec![want(ProbeKind::I, part, element(e), Vec::new())])
}

/// A `v`-family word of one or two nodes.
fn voltage(head: &str, args: &[&str], ctx: Ctx) -> Result<Vec<Want>, No> {
    let node = |n: &str| node_path(n);
    match ctx {
        Ctx::Save => Ok(args
            .iter()
            .map(|n| want(ProbeKind::V, AcPart::None, node(n), Vec::new()))
            .collect()),
        Ctx::Print(kind) => {
            let part = match (kind == AnalysisKind::Ac, head) {
                (_, "v") => AcPart::None,
                (true, "vm") => AcPart::Mag,
                (true, "vp") => AcPart::PhaseRad,
                (true, "vr") => AcPart::Re,
                (true, "vdb") => AcPart::Db,
                _ => return Err(No::Untyped),
            };
            Ok(vec![if args.len() == 2 {
                want(ProbeKind::Vdiff, part, node(args[0]), node(args[1]))
            } else {
                want(ProbeKind::V, part, node(args[0]), Vec::new())
            }])
        }
        Ctx::Meas(_) => {
            if args.len() == 2 {
                return Err(No::Invalid); // "no such vector as v(a,b)"
            }
            let part = match (complex(ctx), head) {
                (false, "v") => AcPart::None,
                (true, "v" | "vr") => AcPart::Re,
                (true, "vm") => AcPart::Mag,
                (true, "vp") => AcPart::PhaseRad,
                (true, "vdb") => AcPart::Db,
                _ => return Err(No::Untyped),
            };
            Ok(vec![want(ProbeKind::V, part, node(args[0]), Vec::new())])
        }
    }
}

/// Whether `ctx` is an ac or sp measure: its values are read by part (`get_value`); a device
/// quantity there is not typed.
fn complex(ctx: Ctx) -> bool {
    matches!(ctx, Ctx::Meas(AnalysisKind::Ac | AnalysisKind::Sp))
}

/// A `Want`.
fn want(kind: ProbeKind, part: AcPart, a: Vec<String>, b: Vec<String>) -> Want {
    Want { kind, part, a, b }
}

/// Whether a segment is an instance name ngspice flattens (`x…`).
fn x_led(s: &str) -> bool {
    s.as_bytes()
        .first()
        .is_some_and(|c| c.eq_ignore_ascii_case(&b'x'))
}
