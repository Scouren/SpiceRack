//! The plan's rows → their statements: a sweep `Loop` → `sweep name target range` then its
//! body one level deeper (a sweep binds the next analysis or sweep, VD cmd-sweep); a Monte Carlo
//! `Loop` → `mc name samples= [seed=] [lh=0]`, its body, `endmc` (VD cmd-analysis-mc, `lh=1`
//! the default); `Alter` → `alter instance("x1:r1") p=v` / `alter model("m") p=v` (VD
//! cmd-alter); `Elaborate` → `elaborate circuit("a", …)` (VD cmd-elaboration); `Option` →
//! `options key=value` (a flag as its key's value: `tran_method="trap"`, `op_skipinitial=1`);
//! `Env` → `options temp=… tnom=…` (VD cmd-options-set). The inverses of `read_run`.
//!
//! Contents: `loop_`, `alter`, `elaborate`, `option`, `env`, `option_word` (crate-private)

use super::analyses::{range, target};
use super::{body, name_of, quote};
use crate::dialect::vacask::expr_write::write_expr;
use crate::dialect::vacask::name::{self, Place};
use crate::dialect::vacask::write::Writer;
use crate::ir;

/// Loop `at`: its head line, its body, and `endmc` for a Monte Carlo loop.
pub(super) fn loop_(w: &mut Writer<'_, '_>, at: ir::ItemRef) {
    let d = w.design;
    let row: ir::Loop = d.row(at.index);
    assert!(!row.name.is_none(), "FEATURES lacks LoopUnnamed");
    let name = d.str(row.name).to_string();
    w.line();
    match row.kind {
        ir::LoopKind::Sweep => {
            let s: ir::plan::SweepLoop = d.detail(row.detail);
            w.out.push_str("sweep ");
            name::spell(&name, Place::Other, w.out);
            target(d, s.target, w.out);
            range(d, s.sweep, w.out);
            w.out.push('\n');
            // A sweep inside a sweep stands on the line below it, unindented (VD cmd-sweep
            // "Nested sweeps"); the analysis one level deeper.
            match d.items(row.body) {
                [only] if is_sweep(d, *only) => {
                    w.depth -= 1;
                    body(w, row.body);
                    w.depth += 1;
                }
                _ => body(w, row.body),
            }
        }
        ir::LoopKind::MonteCarlo => {
            let m: ir::plan::MonteCarlo = d.detail(row.detail);
            w.out.push_str("mc ");
            name::spell(&name, Place::Other, w.out);
            w.out.push_str(" samples=");
            write_expr(d, m.runs, w.out);
            if !m.seed.is_none() {
                w.out.push_str(" seed=");
                write_expr(d, m.seed, w.out);
            }
            if m.sampler == ir::plan::Sampler::Random {
                w.out.push_str(" lh=0");
            }
            w.out.push('\n');
            body(w, row.body);
            w.line();
            w.out.push_str("endmc\n");
        }
        ir::LoopKind::Table => unreachable!("FEATURES lacks LoopTable: legalize refused it"),
    }
}

/// An `Alter` row.
pub(super) fn alter(w: &mut Writer<'_, '_>, row: ir::Alter) {
    let d = w.design;
    let t = row.target;
    w.line();
    let out = &mut *w.out;
    match t.kind {
        ir::TargetKind::DeviceParam => {
            let mut path = name_of(d, t.path);
            if !path.is_empty() {
                path.push(':');
            }
            path.push_str(d.str(t.a.name));
            out.push_str("alter instance(");
            quote(&path, out);
        }
        ir::TargetKind::ModelParam => {
            out.push_str("alter model(");
            quote(d.str(t.a.name), out);
        }
        // R70: the top-level instance is the design, its parameters the top level's.
        ir::TargetKind::Param => out.push_str("alter instance(\"__topinst__\""),
        k => unreachable!("FEATURES lacks an alter of kind {k:?}"),
    }
    out.push_str(") ");
    let key = match t.kind {
        ir::TargetKind::Param => t.a.name,
        _ => t.b,
    };
    name::spell(d.str(key), Place::Other, out);
    out.push('=');
    write_expr(d, row.value, out);
    out.push('\n');
}

/// An `Elaborate` row.
pub(super) fn elaborate(w: &mut Writer<'_, '_>, row: ir::Elaborate) {
    let d = w.design;
    w.line();
    w.out.push_str("elaborate circuit(");
    for (k, r) in d.refs(row.circuits).iter().enumerate() {
        if k > 0 {
            w.out.push_str(", ");
        }
        quote(d.str(r.name), w.out);
    }
    w.out.push_str(")\n");
}

/// An `Option` row.
pub(super) fn option(w: &mut Writer<'_, '_>, row: ir::OptionRow) {
    let d = w.design;
    w.line();
    w.out.push_str("options ");
    match row.key {
        ir::OptionKey::MethodTrap => w.out.push_str("tran_method=\"trap\""),
        ir::OptionKey::MethodEuler => w.out.push_str("tran_method=\"euler\""),
        ir::OptionKey::NoOpIter => w.out.push_str("op_skipinitial=1"),
        k => {
            match k {
                ir::OptionKey::Private => w.out.push_str(d.str(row.private)),
                k => w.out.push_str(option_word(k)),
            }
            w.out.push('=');
            write_expr(d, row.value, w.out);
        }
    }
    w.out.push('\n');
}

/// An `Env` row.
pub(super) fn env(w: &mut Writer<'_, '_>, row: ir::Env) {
    let d = w.design;
    w.line();
    w.out.push_str("options");
    for (key, e) in [("temp", row.temp), ("tnom", row.tnom)] {
        if !e.is_none() {
            w.out.push(' ');
            w.out.push_str(key);
            w.out.push('=');
            write_expr(d, e, w.out);
        }
    }
    w.out.push('\n');
}

/// VACASK's key for a typed option (VD cmd-options-*); a flag's key is the option it sets.
pub(super) fn option_word(k: ir::OptionKey) -> &'static str {
    match k {
        ir::OptionKey::RelTol => "reltol",
        ir::OptionKey::AbsTol => "abstol",
        ir::OptionKey::VnTol => "vntol",
        ir::OptionKey::ChgTol => "chgtol",
        ir::OptionKey::Gmin => "gmin",
        ir::OptionKey::Gshunt => "gshunt",
        ir::OptionKey::Scale => "scale",
        ir::OptionKey::MaxOrd => "tran_maxord",
        ir::OptionKey::Itl1 => "op_itl",
        ir::OptionKey::Itl4 => "tran_itl",
        ir::OptionKey::GminSteps => "homotopy_gminsteps",
        ir::OptionKey::SrcSteps => "homotopy_srcsteps",
        ir::OptionKey::MethodTrap | ir::OptionKey::MethodEuler => "tran_method",
        ir::OptionKey::NoOpIter => "op_skipinitial",
        k => unreachable!("FEATURES lacks Option({k:?}): legalize refused it"),
    }
}

// ---- private ----

/// Whether `at` is a sweep loop.
fn is_sweep(d: &ir::Design, at: ir::ItemRef) -> bool {
    at.kind == ir::ItemKind::Loop && d.row::<ir::Loop>(at.index).kind == ir::LoopKind::Sweep
}
