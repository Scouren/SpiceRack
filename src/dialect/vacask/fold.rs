//! R42: VACASK's spelling of a source with both a DC value and a waveform (`DcRule::Given` with a
//! wave) is a `type="dc"` source line carrying the wave's parameters, plus `alter
//! instance("<name>") type="<wave>"` before each transient-kind analysis that follows a DC-kind
//! one (the first included: the line starts at dc) and `alter instance("<name>") type="dc"`
//! before each DC-kind analysis that follows a transient-kind one. The reader folds exactly that
//! alternation (`folds`, one walk over every CST before lowering): the sources area reads the
//! source as `Given` with the wave, the run area counts the alters as framing. Any other `type`
//! alter reaching a source makes both statements `Private`. The writer's half is `given` (which
//! sources need the alters) and `admits` (a `Given` source with a wave the alternation cannot
//! state). The exact text both areas write is in `rewrite/S4-AREAS.md` ("dc plus type alters").
//!
//! Cites: R42; VD cmd-alter, dev-builtin-src; DIALECT-VACASK §6.1, §6.2.
//!
//! Contents: `Fold`, `Folds` (`Folds::of`, `Folds::is_framing`), `NO_FOLD`, `CODES`, `folds`, `given`, `wave_word`, `admits`
//! (crate-private)

use std::collections::{HashMap, HashSet};

use super::read::{self, Cst};
use crate::ir;
use crate::syntax::ast::{Node, Tag};

/// What the `type` alters reaching one source state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) enum Fold {
    /// The alternation of R42: `DcRule::Given`, the wave the `type` word names (`"pulse"`, …).
    Given(String),
    /// Any other `type` alter: the source line and those alters are `Private`.
    Other,
}

/// Every source a `type` alter reaches, and the alters that are framing of a fold.
#[derive(Debug, Default)]
pub(super) struct Folds {
    /// By the source's name as an alter spells it (`"v1"`, `"x1:v1"`).
    sources: HashMap<String, Fold>,
    /// `(file, alter node)` of every alter a `Given` fold consumes.
    framing: HashSet<(ir::FileId, u32)>,
    /// The alters each walk consumed, by source, before the verdicts are final.
    consumed: Vec<(ir::FileId, u32, String)>,
}

/// A `Given` source with a wave whose alternation VACASK cannot state.
pub(super) const NO_FOLD: ir::Code = ir::Code::new(b'W', 2007);

/// This area's rows.
pub(super) const CODES: &[(ir::Code, ir::Info)] = &[(
    NO_FOLD,
    ir::Info {
        title: "a source with a DC value and a waveform VACASK cannot spell",
        cite: "R42; VD dev-builtin-src (dc= is ignored on a waveform type), cmd-alter",
        explain: "VACASK states a DC value beside a waveform only as a `type=\"dc\"` source and `alter … type=` lines between the analyses. That needs the source at the top of the entry file, a transient analysis, and every analysis at the top of the run, of a DC kind (op, dc, ac, noise, dcinc, dcxf, xf, stb, sp) or a transient.",
        kind: ir::Kind::Refusal,
    },
)];

/// An analysis's side of the alternation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// Reads the source's DC value.
    Dc,
    /// Reads its waveform.
    Tran,
}

/// The state of one source along a control block.
struct Track {
    /// The current `type` word (`"dc"` at the start).
    state: String,
    /// The wave word, once seen.
    wave: Option<String>,
    /// The alter waiting for the next analysis: its node and word.
    pending: Option<(u32, String)>,
    /// The alters consumed.
    alters: Vec<u32>,
    /// Whether the alternation still holds.
    ok: bool,
}

impl Folds {
    /// The verdict for the source `name` (its whole name; `top`: it stands at a file's top).
    /// `None`: no `type` alter reaches it.
    #[allow(dead_code, reason = "read_sources (S4.2) asks it")]
    pub(super) fn of(&self, name: &str, top: bool) -> Option<&Fold> {
        const OTHER: &Fold = &Fold::Other;
        match top {
            true => self.sources.get(name),
            // A nested source is reached by a hierarchical path ending in its name: never a fold.
            false => self
                .sources
                .keys()
                .any(|k| k.rsplit(':').next() == Some(name) && k.contains(':'))
                .then_some(OTHER),
        }
    }

    /// Whether the `alter` statement `n` of `cst` is framing of a `Given` fold (no item).
    #[allow(dead_code, reason = "read_run (S4.3) asks it")]
    pub(super) fn is_framing(&self, cst: &Cst<'_>, n: Node) -> bool {
        self.framing.contains(&(cst.file, n.0))
    }
}

/// The verdicts of every file: one walk over each top-level `control` block.
pub(super) fn folds(csts: &[(Cst<'_>, ir::ScopeId)]) -> Folds {
    let mut out = Folds::default();
    let mut dc_lines: HashSet<String> = HashSet::new();
    let mut seen: HashSet<String> = HashSet::new();
    for (cst, _) in csts {
        let ast = &cst.ast;
        for &s in ast.fields(ast.root()) {
            let n = Node(s);
            match ast.tag(n) {
                Tag::Instance if type_word(cst, n).is_none_or(|w| w == "dc") => {
                    dc_lines.insert(read::field_name(cst, ast.header(n).name));
                }
                Tag::Control => walk(cst, n, &mut out, &mut seen),
                _ => {}
            }
        }
    }
    // A fold needs its source's line at a top, of type dc.
    for (name, fold) in &mut out.sources {
        if !dc_lines.contains(name) {
            *fold = Fold::Other;
        }
    }
    let given: HashSet<&String> = out
        .sources
        .iter()
        .filter(|(_, f)| matches!(f, Fold::Given(_)))
        .map(|(k, _)| k)
        .collect();
    let keep: Vec<(ir::FileId, u32, String)> = std::mem::take(&mut out.consumed)
        .into_iter()
        .filter(|(_, _, name)| given.contains(name))
        .collect();
    out.framing = keep.into_iter().map(|(f, n, _)| (f, n)).collect();
    out
}

/// The top-level sources of `top` that need R42's alters, each with its wave's `type` word,
/// in item order (the order the run area writes their alters in).
#[allow(dead_code, reason = "write_run (S4.3) writes the alters")]
pub(super) fn given(design: &ir::Design, top: ir::ScopeId) -> Vec<(ir::ItemRef, &'static str)> {
    design
        .items(top)
        .iter()
        .filter(|at| at.kind == ir::ItemKind::Element)
        .filter_map(|&at| given_wave(design, at).map(|w| (at, w)))
        .collect()
}

/// VACASK's `type` word of a wave kind (VD dev-builtin-src), or `None` for one it lacks.
pub(super) fn wave_word(kind: ir::WaveKind) -> Option<&'static str> {
    Some(match kind {
        ir::WaveKind::Pulse => "pulse",
        ir::WaveKind::Sin => "sine",
        ir::WaveKind::Exp => "exp",
        ir::WaveKind::Pwl => "pwl",
        ir::WaveKind::Am => "am",
        // vc-sources' run: VACASK's fm is exact for the one Sffm law and pre FEATURES holds.
        ir::WaveKind::Sffm => "fm",
        _ => return None,
    })
}

/// Appends [`NO_FOLD`] for each `Given` source with a wave the alternation cannot state: not at
/// the entry file's top, or in a design whose run has no transient, an analysis below the top or
/// of another kind.
pub(super) fn admits(design: &ir::Design, diags: &mut ir::Diags) {
    let entry = design.file(design.entry()).top;
    let mut tran = false;
    let mut runnable = true;
    for (at, scope) in design.reachable() {
        if at.kind != ir::ItemKind::Analysis {
            continue;
        }
        let row: ir::Analysis = design.row(at.index);
        match kind_of(row.kind) {
            Some(Kind::Tran) => tran = true,
            Some(Kind::Dc) => {}
            None => runnable = false,
        }
        runnable &= scope == entry;
    }
    for (at, scope) in design.reachable() {
        if at.kind != ir::ItemKind::Element || !needs_fold(design, at) {
            continue;
        }
        let spelled = scope == entry && tran && runnable && given_wave(design, at).is_some();
        if !spelled {
            diags.push_at(NO_FOLD, design, at, 0);
        }
    }
}

// ---- private ----

/// The side of an IR analysis kind, or `None` for one the alternation does not cover.
fn kind_of(kind: ir::AnalysisKind) -> Option<Kind> {
    use ir::AnalysisKind as A;
    match kind {
        A::Op | A::Dc | A::Ac | A::Noise | A::DcXf | A::Xf | A::Stb | A::Sp | A::DcInc => {
            Some(Kind::Dc)
        }
        A::Tran => Some(Kind::Tran),
        _ => None,
    }
}

/// The side of a VACASK analysis word (VD cmd-analysis-*), or `None`.
fn word_kind(word: &str) -> Option<Kind> {
    match word {
        "op" | "dcinc" | "dcxf" | "ac" | "acxf" | "noise" | "acstb" | "acsp" => Some(Kind::Dc),
        "tran" => Some(Kind::Tran),
        _ => None,
    }
}

/// Whether element `at` is a source with a given DC value and a wave.
fn needs_fold(design: &ir::Design, at: ir::ItemRef) -> bool {
    let row: ir::Element = design.row(at.index);
    matches!(row.kind, ir::ElemKind::VSource | ir::ElemKind::ISource) && {
        let src = design.source(&row);
        src.dc_rule == ir::source::DcRule::Given && !src.wave.is_none()
    }
}

/// The `type` word of element `at`'s wave when it needs a fold VACASK can spell.
fn given_wave(design: &ir::Design, at: ir::ItemRef) -> Option<&'static str> {
    if !needs_fold(design, at) {
        return None;
    }
    let row: ir::Element = design.row(at.index);
    wave_word(design.source(&row).wave.kind)
}

/// The `type="…"` word of instance `n`, if given.
fn type_word(cst: &Cst<'_>, n: Node) -> Option<String> {
    let ast = &cst.ast;
    ast.fields(n)
        .iter()
        .map(|&f| Node(f))
        .filter(|&f| ast.tag(f) == Tag::Assign)
        .find(|&f| read::field_name(cst, ast.lhs(f)) == "type")
        .map(|f| read::field_name(cst, ast.rhs(f)))
}

/// One `type` alter: the sources it names and its word; `None` for any other alter.
fn type_alter(cst: &Cst<'_>, n: Node) -> Option<(Vec<String>, Option<String>)> {
    let ast = &cst.ast;
    if ast.tag(n) != Tag::Card || cst.tokens.text(ast.main_token(n)) != "alter" {
        return None;
    }
    let fields = ast.fields(n);
    let (&head, rest) = fields.split_first()?;
    if ast.tag(Node(head)) != Tag::Word || read::field_name(cst, head) != "instance" {
        return None;
    }
    let names: Vec<String> = rest
        .iter()
        .take_while(|&&f| ast.tag(Node(f)) == Tag::Str)
        .map(|&f| read::field_name(cst, f))
        .collect();
    let assigns = &rest[names.len()..];
    // Only an `Assign` has a key; a stray field after them (`alter instance("r1", r=2k "r2")`,
    // found by the mutation law) is none.
    let is_type = |f: &u32| {
        ast.tag(Node(*f)) == Tag::Assign && read::field_name(cst, ast.lhs(Node(*f))) == "type"
    };
    if !assigns.iter().any(is_type) {
        return None;
    }
    // Only `type` and one source: anything else is no fold.
    let word = match (assigns, names.len()) {
        ([only], 1) => Some(read::field_name(cst, ast.rhs(Node(*only)))),
        _ => None,
    };
    Some((names, word))
}

/// Walks control block `n`: every `type` alter's source gets a verdict.
fn walk(cst: &Cst<'_>, n: Node, out: &mut Folds, seen: &mut HashSet<String>) {
    let ast = &cst.ast;
    let mut tracks: HashMap<String, Track> = HashMap::new();
    let body = ast.body(n).to_vec();
    // Nested blocks are searched for alters only: one there is no fold. Only a block's body is
    // walked (as `read::read`'s declare walk does): a leaf statement's body slot is no list on
    // a recovered parse (an unclosed `/*` in a control block, dialect_mutants read/run/misc.sim
    // seed 1, made this walk read a garbage list and grow without end).
    let block = |s: u32| {
        s != crate::syntax::ast::NONE
            && matches!(
                ast.tag(Node(s)),
                Tag::Subckt | Tag::Section | Tag::If | Tag::Else | Tag::Control
            )
    };
    let mut nested: Vec<u32> = body.iter().copied().filter(|&s| block(s)).collect();
    let mut found: Vec<u32> = Vec::new();
    // Bounded: each block is taken once (a set of the blocks taken guards a cycle).
    let mut taken: HashSet<u32> = HashSet::new();
    while let Some(s) = nested.pop() {
        if !taken.insert(s) {
            continue;
        }
        for &c in ast.body(Node(s)) {
            match block(c) {
                true => nested.push(c),
                false if c != crate::syntax::ast::NONE => found.push(c),
                false => {}
            }
        }
    }
    for s in found {
        if let Some((names, _)) = type_alter(cst, Node(s)) {
            for name in names {
                track(&mut tracks, &name).ok = false;
            }
        }
    }
    // Every source an alter of the block names is tracked from the block's first statement.
    for &s in &body {
        for name in type_alter(cst, Node(s)).map(|(n, _)| n).unwrap_or_default() {
            track(&mut tracks, &name);
        }
    }
    for &s in &body {
        let m = Node(s);
        if let Some((names, word)) = type_alter(cst, m) {
            for name in &names {
                let t = track(&mut tracks, name);
                match (&word, t.pending.is_some()) {
                    (Some(w), false) => t.pending = Some((s, w.clone())),
                    _ => t.ok = false,
                }
            }
            continue;
        }
        if ast.tag(m) != Tag::Analysis {
            continue;
        }
        // An analysis with no kind word (VACASK rejects it) is no side of the alternation.
        let master = ast.header(m).master;
        let kind = (master != crate::syntax::ast::NONE)
            .then(|| word_kind(&read::field_name(cst, master)))
            .flatten();
        for t in tracks.values_mut() {
            step(t, kind);
        }
    }
    for (name, t) in tracks {
        let given = t.ok && t.pending.is_none() && t.wave.is_some();
        let fold = match (given, seen.insert(name.clone())) {
            (true, true) => {
                out.consumed
                    .extend(t.alters.iter().map(|&a| (cst.file, a, name.clone())));
                Fold::Given(t.wave.expect("checked"))
            }
            // A source altered in two control blocks (two files) is no fold.
            _ => Fold::Other,
        };
        out.sources.insert(name, fold);
    }
}

/// The track of `name`, new at the line's state.
fn track<'t>(tracks: &'t mut HashMap<String, Track>, name: &str) -> &'t mut Track {
    tracks.entry(name.to_string()).or_insert_with(|| Track {
        state: "dc".to_string(),
        wave: None,
        pending: None,
        alters: Vec::new(),
        ok: true,
    })
}

/// One analysis of side `kind` seen by track `t`: its pending alter must switch to that side.
fn step(t: &mut Track, kind: Option<Kind>) {
    let Some(kind) = kind else {
        t.ok = false;
        return;
    };
    if let Some((node, word)) = t.pending.take() {
        let switches = word != t.state;
        let side = match word == "dc" {
            true => Kind::Dc,
            false => Kind::Tran,
        };
        let same_wave = word == "dc" || t.wave.as_ref().is_none_or(|w| *w == word);
        t.ok &= switches && side == kind && same_wave;
        if word != "dc" {
            t.wave = Some(word.clone());
        }
        t.state = word;
        t.alters.push(node);
    }
    let reads_wave = t.state != "dc";
    t.ok &= reads_wave == (kind == Kind::Tran);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::{Lang, parse, tokenize};

    /// The verdict for `v1` and the number of framing alters, for a deck whose source line is
    /// `line` and whose control block is `control`.
    fn verdict(line: &str, control: &str) -> (Option<Fold>, usize) {
        let text = format!("t\n{line}\ncontrol\n{control}endc\n");
        let lex = Lang::VACASK.lexicon().expect("a lexicon");
        let tokens = tokenize(&text, lex);
        let ast = parse(&tokens, lex);
        let cst = Cst {
            tokens,
            ast,
            shift: 0,
            file: ir::FileId::NONE,
        };
        let folds = folds(&[(cst, ir::ScopeId::NONE)]);
        (folds.of("v1", true).cloned(), folds.framing.len())
    }

    /// Oracle: R42 (the alternation is exactly a wave before each transient-kind analysis that
    /// follows a DC-kind one, the line starting at dc, and dc before each DC-kind one that
    /// follows a transient; anything else altering `type` is `Other`; no alter is no verdict).
    #[test]
    fn only_the_plans_alternation_folds() {
        let dc = "v1 (a 0) vs type=\"dc\" dc=-6 val0=0 val1=1";
        let to = |w: &str| format!("  alter instance(\"v1\") type=\"{w}\"\n");
        let (op, tran) = ("  analysis op1 op\n", "  analysis tr1 tran stop=1m\n");
        let given = Some(Fold::Given("pulse".to_string()));
        let cases = [
            (
                dc,
                format!("{op}{}{tran}{}{op}", to("pulse"), to("dc")),
                (given.clone(), 2),
            ),
            (dc, format!("{}{tran}", to("pulse")), (given, 1)),
            (dc, format!("{op}{tran}"), (None, 0)),
            (
                dc,
                format!("{tran}{}{tran}", to("pulse")),
                (Some(Fold::Other), 0),
            ),
            (
                dc,
                format!("{}{op}{}{tran}", to("dc"), to("pulse")),
                (Some(Fold::Other), 0),
            ),
            (dc, format!("{op}{}", to("pulse")), (Some(Fold::Other), 0)),
            (
                dc,
                format!("  alter instance(\"v1\") type=\"pulse\" val1=2\n{tran}"),
                (Some(Fold::Other), 0),
            ),
            (
                "v1 (a 0) vs type=\"pulse\" val0=0 val1=1",
                format!("{}{tran}", to("pulse")),
                (Some(Fold::Other), 0),
            ),
            (
                dc,
                format!("{}{tran}{}  analysis hb1 hb\n", to("pulse"), to("dc")),
                (Some(Fold::Other), 0),
            ),
            // A stray string after an assignment (the mutation law's `alter instance("r1",
            // r=2k "r2")`): no `type` alter, and no panic.
            (
                dc,
                format!("  alter instance(\"v1\") val1=2 \"v2\"\n{tran}"),
                (None, 0),
            ),
        ];
        for (line, control, want) in cases {
            assert_eq!(verdict(line, &control), want, "{control}");
        }
    }
}
