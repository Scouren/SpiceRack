//! `options k=v …` → typed `Option` rows and `Env` (temp, tnom), one `Hint` opaque for the
//! line's solver knobs, one `Private` opaque for its accuracy targets VACASK alone has, one
//! `Inert` opaque for its output-only keys; each group stands where its first key stood. An
//! unknown key refuses the line (run e6: "Parameter 'foo' not found.").
//!
//! The key table is VACASK's documented option set (VD cmd-options-*); the typed keys are
//! DIALECT-VACASK §3.4's (R43: only `trap` and `euler` are typed methods, `tran_lteratio` a
//! hint). Every row holds `FromHere` (A14).
//!
//! Contents: `options` (crate-private)

use super::{OPTION_PRIVATE, REJECTED, assign, check_value, field_text, push_opaque, string};
use crate::dialect::vacask::expr_read::{self, Context};
use crate::dialect::vacask::number;
use crate::dialect::vacask::read::{self, Lower};
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// Keys VACASK reads as the IR's portable options (VD cmd-options-tol, -temp, -tran, -op,
/// -homotopy).
const TYPED: &[(&str, ir::OptionKey)] = &[
    ("reltol", ir::OptionKey::RelTol),
    ("abstol", ir::OptionKey::AbsTol),
    ("vntol", ir::OptionKey::VnTol),
    ("chgtol", ir::OptionKey::ChgTol),
    ("gmin", ir::OptionKey::Gmin),
    ("gshunt", ir::OptionKey::Gshunt),
    ("scale", ir::OptionKey::Scale),
    ("tran_maxord", ir::OptionKey::MaxOrd),
    ("op_itl", ir::OptionKey::Itl1),
    ("tran_itl", ir::OptionKey::Itl4),
    ("homotopy_gminsteps", ir::OptionKey::GminSteps),
    ("homotopy_srcsteps", ir::OptionKey::SrcSteps),
];

/// Solver knobs: tune convergence or step control, name no accuracy target (R19).
const HINTS: &[&str] = &[
    "nr_damping",
    "nr_conviter",
    "nr_residualcheck",
    "nr_force",
    "nr_nsforce",
    "nr_bypass",
    "nr_convtol",
    "nr_bypasstol",
    "nr_contbypass",
    "matrixcheck",
    "rhscheck",
    "solutioncheck",
    "rcondcheck",
    "omp_evbatches",
    "omp_evsched",
    "omp_evchunk",
    "op_itlcont",
    "op_skipinitial",
    "op_homotopy",
    "op_srchomotopy",
    "op_nsiter",
    "homotopy_startgmin",
    "homotopy_maxgmin",
    "homotopy_mingmin",
    "homotopy_gminfactor",
    "homotopy_maxgminfactor",
    "homotopy_mingminfactor",
    "homotopy_srcstep",
    "homotopy_srcscale",
    "homotopy_minsrcstep",
    "hb_itl",
    "hb_itlcont",
    "hb_skipinitial",
    "hb_homotopy",
    "hb_nsiter",
    "tdsolver",
    "smsigsolver",
    "hbsolver",
    "qpsmsigsolver",
    "tran_fs",
    "tran_ffmax",
    "tran_fbr",
    "tran_rmax",
    "tran_minpts",
    "tran_ft",
    "tran_predictor",
    "tran_redofactor",
    "tran_lteratio",
    "tran_laggednoise",
    "tran_extraoct",
    "tran_noiselte",
    "pss_itl",
];

/// Keys only VACASK has that scale an accuracy target or change the equations (R19).
const PRIVATE: &[&str] = &[
    "tolscale",
    "tolmode",
    "fluxtol",
    "relref",
    "relrefsol",
    "relrefres",
    "relreflte",
    "minr",
    "unknownparam",
    "strictforce",
    "tran_method",
    "tran_xmu",
    "tran_trapltefilter",
    "tran_spicelte",
    "pss_minpts",
    "pss_tolscale",
    "homotopy_sourcefactor",
];

/// Keys that move no number: the raw file's format and cleanup, save binding, debug output.
const INERT: &[&str] = &[
    "rawfile",
    "strictoutput",
    "strictsave",
    "sweep_pointmarker",
    "sweep_debug",
    "op_debug",
    "nr_debug",
    "hb_debug",
    "homotopy_debug",
    "smsig_debug",
    "pss_debug",
    "tran_debug",
    "tran_noisedebug",
];

/// What one key of a line becomes.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    /// A typed option with a value.
    Typed(ir::OptionKey),
    /// A flag option (`tran_method="trap"`, `op_skipinitial=1`).
    Flag(ir::OptionKey),
    /// `temp` or `tnom`.
    Env,
    /// Into the line's opaque of that class.
    Opaque(ir::OpaqueClass),
}

/// `options k=v …`.
pub(super) fn options(l: &mut Lower<'_, '_, '_>, n: Node) {
    let fields = l.cst.ast.fields(n).to_vec();
    let mut keys = Vec::with_capacity(fields.len());
    for &f in &fields {
        let Some((key, value)) = assign(l, f) else {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
        };
        let Some(class) = classify(l, &key, value) else {
            return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
        };
        keys.push((key, value, class));
    }
    if keys.is_empty() {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    }
    for (_, value, class) in &keys {
        if matches!(class, Class::Typed(_) | Class::Env)
            && let Err(r) = check_value(l, *value)
        {
            return read::opaque(l, n, r.class, Some(r.code));
        }
    }
    // Each group at its first key: the env, then each opaque class, by first appearance.
    let mut done_env = false;
    let mut done: Vec<ir::OpaqueClass> = Vec::new();
    let span = l.stmt;
    for (k, (key, value, class)) in keys.iter().enumerate() {
        match *class {
            Class::Typed(o) => {
                let e = expr_read::expr(l, *value, Context::Param).expect("checked");
                l.b.option(l.scope, o, e, span);
            }
            Class::Flag(o) => {
                l.b.option(l.scope, o, ir::ExprId::NONE, span);
            }
            Class::Env if !done_env => {
                done_env = true;
                let mut env = [ir::ExprId::NONE; 2];
                for (key2, value2, class2) in &keys[k..] {
                    if *class2 == Class::Env {
                        let at = usize::from(key2 == "tnom");
                        env[at] = expr_read::expr(l, *value2, Context::Param).expect("checked");
                    }
                }
                let _ = key;
                l.b.env(l.scope, env[0], env[1], span);
            }
            Class::Env => {}
            Class::Opaque(c) if !done.contains(&c) => {
                done.push(c);
                let mut text = String::from("options");
                for (j, (_, _, class2)) in keys.iter().enumerate() {
                    if *class2 == Class::Opaque(c) {
                        text.push(' ');
                        text.push_str(field_text(l, n, j));
                    }
                }
                let code = (c == ir::OpaqueClass::Private).then_some(OPTION_PRIVATE);
                push_opaque(l, c, &text, code);
            }
            Class::Opaque(_) => {}
        }
    }
}

// ---- private ----

/// What `key` (with value node `value`) becomes; `None` for a key VACASK lacks.
fn classify(l: &Lower<'_, '_, '_>, key: &str, value: u32) -> Option<Class> {
    if let Some(&(_, o)) = TYPED.iter().find(|(k, _)| *k == key) {
        return Some(Class::Typed(o));
    }
    if key == "temp" || key == "tnom" {
        return Some(Class::Env);
    }
    if key == "tran_method" {
        return Some(match string(l, value).as_deref() {
            Some("trap") => Class::Flag(ir::OptionKey::MethodTrap),
            Some("euler") => Class::Flag(ir::OptionKey::MethodEuler),
            _ => Class::Opaque(ir::OpaqueClass::Private),
        });
    }
    if key == "op_skipinitial" && is_int_one(l, value) {
        return Some(Class::Flag(ir::OptionKey::NoOpIter));
    }
    let class = if HINTS.contains(&key) {
        ir::OpaqueClass::Hint
    } else if PRIVATE.contains(&key) {
        ir::OpaqueClass::Private
    } else if INERT.contains(&key) {
        ir::OpaqueClass::Inert
    } else {
        return None;
    };
    Some(Class::Opaque(class))
}

/// Whether `value` is the integer literal 1.
fn is_int_one(l: &Lower<'_, '_, '_>, value: u32) -> bool {
    let ast = &l.cst.ast;
    let n = Node(value);
    ast.tag(n) == Tag::Num
        && number::lex(l.cst.tokens.text(ast.main_token(n)))
            .is_ok_and(|x| x.int && x.literal.mantissa == 1)
}
