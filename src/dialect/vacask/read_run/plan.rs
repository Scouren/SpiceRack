//! The control block's other statements: `mc name samples= …` (a `Loop` of kind Monte Carlo,
//! its body the statements up to `endmc`), `alter`, `elaborate`, `clear`, `var`,
//! `print`/`abort`/`postprocess`.
//!
//! Rules: `mc`'s `seed` absent is VACASK's own stream (seed 0), `lh` absent is Latin hypercube
//! (VD cmd-analysis-mc); after the loop the nominal values are back (`after: Restore`); an
//! `alter instance("a", "b") p=v q=w` is one `Alter` per (name, parameter), persisting (VD
//! cmd-alter; run e2); `elaborate circuit("a", …)` is an `Elaborate` of those subcircuits,
//! `elaborate circuit` and `elaborate changes` happen anyway (Inert, VD cmd-elaboration);
//! `clear saves` drops every save before it, the analyses after it back on their defaults
//! (run sv2: `Reset` of the saves, A27), `clear options`/`variables`/`clear` are Private;
//! `var` is Private until the IR has a circuit variable (R43); `print`, `abort`,
//! `postprocess` move no number (Inert, C K10).
//!
//! Contents: `mc`, `statement` (crate-private)

use super::{
    ALTER_TYPE, CLEAR_STATE, ELABORATE_TOP, MC_FIELD, REJECTED, SWEEP_WAVE, TOP_INSTANCE, VARIABLE,
    assign, bare_name, bind_top_param, check_value, string, top_param,
};
use crate::dialect::vacask::expr_read::{self, Context};
use crate::dialect::vacask::fold;
use crate::dialect::vacask::read::{self, Lower};
use crate::dialect::vacask::read_elements::Aimed;
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// A statement's keyword, its string names and its assignments (`alter`, `elaborate`).
type Shape = (String, Vec<String>, Vec<(String, u32)>);

/// `mc name samples=n [seed=s] [lh=b]`: the loop's body becomes the current scope (`Some(false)`);
/// kept as text with `strict` or `debug` (`Some(true)`); refused (`None`).
pub(super) fn mc(l: &mut Lower<'_, '_, '_>, n: Node) -> Option<bool> {
    let fields = l.cst.ast.fields(n).to_vec();
    let bad = |l: &mut Lower<'_, '_, '_>| {
        read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
        None
    };
    let Some((&name, rest)) = fields.split_first() else {
        return bad(l);
    };
    if l.cst.ast.tag(Node(name)) != Tag::Word {
        return bad(l);
    }
    if !bare_name(l, name) {
        return bad(l);
    }
    let name = read::field_name(l.cst, name);
    let mut pairs = Vec::new();
    for &f in rest {
        match assign(l, f) {
            Some(p) => pairs.push(p),
            None => return bad(l),
        }
    }
    let get = |k: &str| pairs.iter().find(|(x, _)| x == k).map(|&(_, v)| v);
    if pairs
        .iter()
        .any(|(k, _)| !["samples", "seed", "lh", "strict", "debug"].contains(&k.as_str()))
    {
        return bad(l);
    }
    let Some(samples) = get("samples") else {
        return bad(l);
    };
    if get("strict").is_some() || get("debug").is_some() {
        read::opaque(l, n, ir::OpaqueClass::Private, Some(MC_FIELD));
        return Some(true);
    }
    let lh = match get("lh") {
        None => true,
        Some(v) => match flag(l, v) {
            Some(b) => b,
            None => {
                read::opaque(l, n, ir::OpaqueClass::Private, Some(MC_FIELD));
                return Some(true);
            }
        },
    };
    let seed = get("seed");
    // R68, A42: `samples` and `seed` are analysis arguments (settle run/mc_param: "Variable or
    // constant 'n' not defined." for a parameter; a `var` runs, mc_var).
    let given: Vec<u32> = [Some(samples), seed].into_iter().flatten().collect();
    if let Err(w) = super::sweep::check_arg(l, &given) {
        read::opaque(l, n, w.class, w.code);
        // Kept as text with its body when Private (a `var`'s read); refused like any bad `mc`
        // line when invalid, its body read as the statements it holds.
        return (w.class == ir::OpaqueClass::Private).then_some(true);
    }
    let runs = expr_read::expr(l, samples, Context::Param).expect("checked");
    let seed = seed.map_or(ir::ExprId::NONE, |s| {
        expr_read::expr(l, s, Context::Param).expect("checked")
    });
    let row = ir::plan::MonteCarlo {
        runs,
        seed,
        samples: ir::List::EMPTY,
        variations: ir::plan::Variations::Process,
        sampler: match lh {
            true => ir::plan::Sampler::LatinHypercube,
            false => ir::plan::Sampler::Random,
        },
        nominal: false,
    };
    let span = l.stmt;
    let (_, body) =
        l.b.loop_(l.scope, Some(&name), row, ir::LoopEnd::Restore, span);
    l.scope = body;
    Some(false)
}

/// Any other control statement, by its head `word`.
pub(super) fn statement(l: &mut Lower<'_, '_, '_>, n: Node, word: &str) {
    match word {
        "alter" => alter(l, n),
        "elaborate" => elaborate(l, n),
        "clear" => clear(l, n),
        "var" => read::opaque(l, n, ir::OpaqueClass::Private, Some(VARIABLE)),
        "print" | "abort" => read::opaque(l, n, ir::OpaqueClass::Inert, None),
        w if w.starts_with("postprocess") => read::opaque(l, n, ir::OpaqueClass::Inert, None),
        _ => read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED)),
    }
}

// ---- private ----

/// The integer literal 0 or 1 at `v` as a flag, or `None`.
fn flag(l: &Lower<'_, '_, '_>, v: u32) -> Option<bool> {
    let ast = &l.cst.ast;
    match (ast.tag(Node(v)), l.cst.tokens.text(ast.main_token(Node(v)))) {
        (Tag::Num, "0") => Some(false),
        (Tag::Num, "1") => Some(true),
        _ => None,
    }
}

/// The fields of `n`: its first `Word` (the keyword), the string names after it, and its
/// assignments; `None` when they come in another order or shape.
fn split(l: &Lower<'_, '_, '_>, n: Node) -> Option<Shape> {
    let fields = l.cst.ast.fields(n).to_vec();
    let (&first, rest) = fields.split_first()?;
    if l.cst.ast.tag(Node(first)) != Tag::Word {
        return None;
    }
    let word = read::field_name(l.cst, first);
    let names: Vec<String> = rest.iter().map_while(|&f| string(l, f)).collect();
    let pairs: Option<Vec<(String, u32)>> =
        rest[names.len()..].iter().map(|&f| assign(l, f)).collect();
    Some((word, names, pairs?))
}

/// Whether every instance alter `n` names is a top-level source the design already holds as
/// `DcRule::Given` with a wave (`fold::given`).
fn framed(l: &Lower<'_, '_, '_>, n: Node) -> bool {
    let Some((_, names, _)) = split(l, n) else {
        return false;
    };
    let design = l.b.design();
    let top = design.file(l.cst.file).top;
    let given = fold::given(design, top);
    names.iter().all(|name| {
        given
            .iter()
            .any(|(at, _)| design.str(design.row::<ir::Element>(at.index).name) == name)
    })
}

/// `alter instance("a", …) p=v …` / `alter model("m", …) p=v …`.
fn alter(l: &mut Lower<'_, '_, '_>, n: Node) {
    // R42: an alter a `Given` fold consumed is framing of its source, no item (`fold.rs`),
    // once the source stands in the design as `Given` with its wave (the writer re-emits the
    // framing from it); else the alter is kept as text below.
    if l.folds.is_framing(l.cst, n) && framed(l, n) {
        return;
    }
    let shape = split(l, n).filter(|(w, names, pairs)| {
        matches!(w.as_str(), "instance" | "model") && !names.is_empty() && !pairs.is_empty()
    });
    let Some((word, names, pairs)) = shape else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    };
    if pairs.iter().any(|(k, _)| k == "type") {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(ALTER_TYPE));
    }
    for (_, v) in &pairs {
        if let Err(r) = check_value(l, *v) {
            return read::opaque(l, n, r.class, Some(r.code));
        }
    }
    // R70, R64b: `__topinst__` is the design itself, its parameters the top level's; a dc
    // source's `dc` is its DC value, a passive's `r`/`c`/`l` its value; an instance or a
    // top-level parameter no file declares is VACASK's error (settle run/nosuch_alter,
    // top_nosuch: "Instance not found.", "Parameter 'zz' not found."); the `dc` of a waveform
    // source moves nothing VACASK reads (settle run/sweep_source), kept as text.
    let at = (l.cst.file, l.start(n));
    let aim = |l: &Lower<'_, '_, '_>, name: &str, key: &str| match name == TOP_INSTANCE {
        true if top_param(l, key) => Some(ir::TargetKind::Param),
        true => None,
        false => {
            let mut path: Vec<String> = name.split(':').map(str::to_string).collect();
            let element = path.pop().expect("split gives one part");
            match l.declared.aim(&path, &element, key, at) {
                Aimed::Missing => None,
                Aimed::SourceDc => Some(ir::TargetKind::SourceDc),
                Aimed::SourceWave => Some(ir::TargetKind::Temp),
                Aimed::ElementValue => Some(ir::TargetKind::ElementValue),
                Aimed::Device => Some(ir::TargetKind::DeviceParam),
            }
        }
    };
    let mut kinds = Vec::new();
    if word == "instance" {
        for name in &names {
            for (key, _) in &pairs {
                let Some(kind) = aim(l, name, key) else {
                    return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
                };
                kinds.push(kind);
            }
        }
    }
    // `Temp` stands for a waveform source's `dc` here (no alter targets the temperature).
    if kinds.contains(&ir::TargetKind::Temp) {
        return read::opaque(l, n, ir::OpaqueClass::Private, Some(SWEEP_WAVE));
    }
    let mut kinds = kinds.into_iter();
    let span = l.stmt;
    for name in &names {
        for (key, v) in &pairs {
            let target = match word.as_str() {
                "instance" => match kinds.next().expect("one kind per name and key") {
                    ir::TargetKind::Param => {
                        let t = l.b.target(ir::TargetKind::Param, &[], key, None);
                        bind_top_param(l, t)
                    }
                    kind => {
                        let mut path: Vec<&str> = name.split(':').collect();
                        let element = path.pop().expect("split gives one part");
                        let b = (kind == ir::TargetKind::DeviceParam).then_some(key.as_str());
                        l.b.target(kind, &path, element, b)
                    }
                },
                _ => {
                    let mut t = l.b.target(ir::TargetKind::ModelParam, &[], name, Some(key));
                    let design = l.b.design();
                    let top = design.file(l.cst.file).top;
                    let def = design.lookup(top, ir::DefKind::Model, t.a.name);
                    if !def.is_none() {
                        t.a.def = def;
                    }
                    t
                }
            };
            let value = expr_read::expr(l, *v, Context::Param).expect("checked");
            l.b.alter(l.scope, target, value, span);
        }
    }
}

/// `elaborate circuit[("a", …)] [topdef=…] [topinst=…]` / `elaborate changes`.
fn elaborate(l: &mut Lower<'_, '_, '_>, n: Node) {
    let Some((word, names, pairs)) = split(l, n) else {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    };
    let known = pairs.iter().all(|(k, _)| k == "topdef" || k == "topinst");
    match (word.as_str(), names.is_empty(), pairs.is_empty()) {
        ("changes", true, true) | ("circuit", true, true) => {
            read::opaque(l, n, ir::OpaqueClass::Inert, None)
        }
        ("circuit", _, false) if known => {
            read::opaque(l, n, ir::OpaqueClass::Private, Some(ELABORATE_TOP))
        }
        ("circuit", false, true) => {
            let span = l.stmt;
            let names: Vec<&str> = names.iter().map(String::as_str).collect();
            let at = l.b.elaborate(l.scope, &names, span);
            let top = l.b.design().file(l.cst.file).top;
            for (k, name) in names.iter().enumerate() {
                let id = l.b.name(name);
                let def = l.b.design().lookup(top, ir::DefKind::Subckt, id);
                if !def.is_none() {
                    l.b.bind_ref(at, k as u32, def);
                }
            }
        }
        _ => read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED)),
    }
}

/// `clear [saves|options|variables]`.
fn clear(l: &mut Lower<'_, '_, '_>, n: Node) {
    let fields = l.cst.ast.fields(n).to_vec();
    let words: Vec<String> = fields
        .iter()
        .filter(|&&f| l.cst.ast.tag(Node(f)) == Tag::Word)
        .map(|&f| read::field_name(l.cst, f))
        .collect();
    if words.len() != fields.len() || words.len() > 1 {
        return read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED));
    }
    match words.first().map(String::as_str) {
        Some("saves") => {
            let span = l.stmt;
            let saves = ir::ResetSet::of(&[ir::Restorable::Saves]);
            l.b.reset(l.scope, saves, span);
        }
        None | Some("options" | "variables" | "var") => {
            read::opaque(l, n, ir::OpaqueClass::Private, Some(CLEAR_STATE))
        }
        _ => read::opaque(l, n, ir::OpaqueClass::InvalidInSource, Some(REJECTED)),
    }
}
