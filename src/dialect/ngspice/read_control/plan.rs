//! The deck's script → each statement's operations, in the order ngspice applies them: one walk
//! over every statement's commands in file order, with what ngspice keeps between them (the
//! targets a `reset` would restore, the `alterparam`s waiting for one, the loop words in
//! scope). Text in, operations out (words, not yet values): `push.rs` validates and pushes. A
//! whole-script verdict (`Script`) is an `Err` with its code.
//!
//! Cites: `rewrite/S3-CONTROL.md` §1 (S1–S5, V1–V3), §2 (the verdicts); `rewrite/RULINGS.md`
//! R49 (3), (5); the lead's Q3–Q6 answers.
//!
//! Contents: `pub(super) const UNROLL_MAX: usize`, `pub(super) const MARK: &str`, `pub(super) enum Target`,
//! `pub(super) enum Op`, `pub(super) fn plan(&[Block], card_kinds: &[(String, usize)]) ->
//! Result<Vec<Vec<Op>>, ir::Code>`

use super::args::{
    alter, alterparam, assignments, class_of, is_number, known, repeat_count, subst,
};
use super::script::{Block, Cmd, Node};
use super::table::{
    ANALYSES, CIRCUIT_OPTIONS, EFFECTS, FLOWS, FRONTEND_KEYS, INERT, OWNS, PRIVATE_VARS,
};
use super::{DISPLAY, EFFECT, FLOW, FORM, NO_COMMAND, NO_RESET, OWN, STATE_WITH_CARDS, UNROLL};
use crate::dialect::ngspice::read_run;
use crate::ir;

/// Operations one script may unroll to (a `foreach` or `repeat` not read as a `Loop`); past it
/// the script is `Script` (`UNROLL`). The corpus' largest unrolled ngspice-45 example has 64.
pub(super) const UNROLL_MAX: usize = 4096;

/// A marker standing for a loop's word while its body is read as a template.
pub(super) const MARK: &str = "\u{1}";

/// What an `alter`, `altermod` or `alterparam` changes, by name as written.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Target {
    /// An R, C or L's own value (`alter r1 2k`).
    Element(String),
    /// A V or I source's DC value (`alter v1 1`).
    Source(String),
    /// A named instance parameter (`alter r1 resistance 2k`, `alter @r1[resistance]=2k`).
    Device(String, String),
    /// A model parameter (`altermod dmod is=1e-10`).
    Model(String, String),
    /// A `.param` (`alterparam p=3k`, applied at the next `reset`).
    Param(String),
}

/// One operation, words still text.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Op {
    /// An analysis command: its word and its arguments after substitution.
    Analysis {
        name: String,
        rest: String,
        cmd: Cmd,
    },
    /// From here `target` holds `value` (one word; an `alterparam`'s numparam text).
    Alter {
        target: Target,
        value: String,
        cmd: Cmd,
    },
    /// A script `option`/`set` of simulator options: `(key, value)` in order.
    State {
        keys: Vec<(String, Option<String>)>,
        cmd: Cmd,
    },
    /// `pre_osdi path`.
    Load { path: String, cmd: Cmd },
    /// `save …` in its exact case (before the first analysis, no `reset` in the script).
    Save {
        rest: String,
        holds: ir::Applies,
        cmd: Cmd,
    },
    /// `meas …` in its exact case (the script's one analysis of its kind ran before it, and
    /// the deck has no card a `run` stands for: lead's Q5).
    Meas { rest: String, cmd: Cmd },
    /// `reset`: every class back to the design's own (A28: `Reset { restores: ALL }`).
    Reset { cmd: Cmd },
    /// A command kept as its own opaque.
    Opaque {
        class: ir::OpaqueClass,
        code: ir::Code,
        cmd: Cmd,
    },
    /// An `alterparam`, resolved by [`plan`]: dropped when a `reset` applied it, else an `Inert`
    /// opaque (S2). Never leaves this file.
    Pending { id: usize, cmd: Cmd },
    /// A `foreach` read as the IR loop: `target` set from `var` over `values`, then `body`.
    Loop {
        var: String,
        target: Target,
        values: Vec<String>,
        body: Vec<Op>,
        cmd: Cmd,
    },
}

/// What the walk keeps between commands.
#[derive(Clone, Debug, Default)]
struct Interp {
    /// `alterparam`s waiting for a `reset`: their placeholder id, target and value.
    pending: Vec<(usize, Target, String, Cmd)>,
    /// Whether each placeholder was applied by a `reset`.
    applied: Vec<bool>,
    /// Whether a command changed state (R49 (3)).
    changed: bool,
    /// Whether an analysis ran yet (a `save` after one holds from there).
    analysed: bool,
    /// Whether the script holds a `reset` anywhere (a `save` before it is undone by it).
    resets: bool,
    /// The analysis words the whole script runs, a word twice if it runs more than once (a
    /// loop body counts twice).
    runs: Vec<String>,
    /// The analysis words run so far.
    seen: Vec<String>,
    /// Whether the deck has a dot-card analysis.
    cards: bool,
    /// Each dot-card analysis: its keyword, and how many script statements stand before it.
    card_kinds: Vec<(String, usize)>,
    /// Whether `quit` ended the script.
    quit: bool,
    /// Operations produced so far (the unroll bound).
    count: usize,
}

/// What a `set` assigns.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SetKind {
    /// Only display variables.
    Display,
    /// Only simulator options (`cp_usrset`'s `US_SIMVAR`).
    Simulator,
    /// Anything else.
    Mixed,
}

/// Each statement's operations, or the whole script's `Script` code.
pub(super) fn plan(
    blocks: &[Block],
    card_kinds: &[(String, usize)],
) -> Result<Vec<Vec<Op>>, ir::Code> {
    let cards = !card_kinds.is_empty();
    let mut it = Interp {
        card_kinds: card_kinds.to_vec(),
        cards,
        resets: blocks
            .iter()
            .any(|b| b.nodes.as_ref().is_ok_and(|n| has_reset(n))),
        runs: blocks
            .iter()
            .filter_map(|b| b.nodes.as_ref().ok())
            .flat_map(|n| analysis_words(n, 1))
            .collect(),
        ..Interp::default()
    };
    let mut out = Vec::with_capacity(blocks.len());
    for b in blocks {
        let nodes = b.nodes.as_ref().map_err(|()| FLOW)?;
        let mut ops = Vec::new();
        it.nodes(nodes, &[], &mut ops)?;
        out.push(ops);
    }
    if cards && it.changed {
        return Err(STATE_WITH_CARDS);
    }
    let applied = it.applied;
    Ok(out.into_iter().map(|ops| settle(ops, &applied)).collect())
}

// ---- private ----
impl Interp {
    /// Walks `nodes` with the loop words `env` in scope, appending to `out`.
    fn nodes(
        &mut self,
        nodes: &[Node],
        env: &[(String, String)],
        out: &mut Vec<Op>,
    ) -> Result<(), ir::Code> {
        for n in nodes {
            if self.quit {
                return Err(EFFECT);
            }
            match n {
                Node::Cmd(c) => self.cmd(c, env, out)?,
                Node::Flow(_) => return Err(FLOW),
                Node::Repeat { head, body } => {
                    let n = repeat_count(&subst(head.rest(), env)?).ok_or(FLOW)?;
                    for _ in 0..n {
                        self.nodes(body, env, out)?;
                    }
                }
                Node::Foreach { head, body } => self.foreach(head, body, env, out)?,
            }
            self.count += 1;
            if self.count > UNROLL_MAX || out.len() > UNROLL_MAX {
                return Err(UNROLL);
            }
        }
        Ok(())
    }

    /// A `foreach`: the IR loop when one pass is exactly "set the target from the word, then a
    /// body that reads it only in display commands"; else unrolled, each word substituted.
    fn foreach(
        &mut self,
        head: &Cmd,
        body: &[Node],
        env: &[(String, String)],
        out: &mut Vec<Op>,
    ) -> Result<(), ir::Code> {
        let rest = head.rest();
        let mut parts = rest.split_whitespace();
        let var = parts.next().ok_or(FLOW)?.to_string();
        let words: Vec<String> = parts.map(str::to_string).collect();
        let constant = |w: &String| !w.chars().any(|c| "$`{}*?[]~".contains(c));
        if words.is_empty() || !words.iter().all(constant) {
            return Err(FLOW);
        }
        let saved = self.clone();
        let mut inner = env.to_vec();
        inner.push((var.clone(), MARK.to_string()));
        let mut template = Vec::new();
        if self.nodes(body, &inner, &mut template).is_ok()
            && let Some((target, rest)) = law(&template, &words, &self.applied)
        {
            out.push(Op::Loop {
                var,
                target,
                values: words,
                body: rest,
                cmd: head.clone(),
            });
            return Ok(());
        }
        *self = saved;
        for w in &words {
            let mut inner = env.to_vec();
            inner.push((var.clone(), w.clone()));
            self.nodes(body, &inner, out)?;
        }
        Ok(())
    }

    /// One command.
    fn cmd(
        &mut self,
        c: &Cmd,
        env: &[(String, String)],
        out: &mut Vec<Op>,
    ) -> Result<(), ir::Code> {
        // an unrolled pass: the command's own text gets its loop words, so that a kept opaque
        // means what that pass ran
        let unrolled = Cmd {
            text: known(&c.text, env),
            line: c.line.clone(),
            hash: c.hash,
        };
        let c = &unrolled;
        let word = c.word();
        let opaque = |class, code| Op::Opaque {
            class,
            code,
            cmd: c.clone(),
        };
        let w = word.as_str();
        if let Some(pre) = w.strip_prefix("pre_") {
            if pre == "osdi" {
                out.push(Op::Load {
                    path: subst(c.rest(), env)?,
                    cmd: c.clone(),
                });
            } else if pre == "set" && self.set_kind(c.rest()) == SetKind::Display {
                out.push(opaque(ir::OpaqueClass::Inert, DISPLAY));
            } else {
                out.push(opaque(ir::OpaqueClass::Private, OWN));
            }
            return Ok(());
        }
        match w {
            "run" => self.seen.push("run".to_string()),
            "quit" => {
                self.quit = true;
                out.push(opaque(ir::OpaqueClass::Inert, DISPLAY));
            }
            _ if ANALYSES.contains(&w) => {
                self.analysed = true;
                self.seen.push(word.clone());
                out.push(Op::Analysis {
                    name: word.clone(),
                    rest: subst(c.rest(), env)?,
                    cmd: c.clone(),
                });
            }
            "let" if c.rest().starts_with('@') => return Err(EFFECT),
            _ if INERT.contains(&w) => out.push(opaque(ir::OpaqueClass::Inert, DISPLAY)),
            _ if EFFECTS.contains(&w) => return Err(EFFECT),
            _ if FLOWS.contains(&w) || w == "end" => return Err(FLOW),
            _ if OWNS.contains(&w) => out.push(opaque(ir::OpaqueClass::Private, OWN)),
            "alter" | "altermod" => {
                self.changed = true;
                let rest = subst(c.rest(), env)?;
                match alter(&rest, w == "altermod") {
                    Ok((target, value)) => {
                        out.push(Op::Alter {
                            target,
                            value,
                            cmd: c.clone(),
                        });
                    }
                    Err(code) => out.push(opaque(class_of(code), code)),
                }
            }
            "alterparam" => {
                self.changed = true;
                let rest = subst(c.rest(), env)?;
                match alterparam(&rest) {
                    Ok((target, value)) => {
                        let id = self.applied.len();
                        self.applied.push(false);
                        self.pending.push((id, target, value, c.clone()));
                        out.push(Op::Pending { id, cmd: c.clone() });
                    }
                    Err(code) => out.push(opaque(class_of(code), code)),
                }
            }
            "reset" => {
                self.changed = true;
                // every class back to the design's own (A28); the waiting alterparams after it
                out.push(Op::Reset { cmd: c.clone() });
                for (id, target, value, cmd) in std::mem::take(&mut self.pending) {
                    self.applied[id] = true;
                    out.push(Op::Alter { target, value, cmd });
                }
            }
            "option" | "options" | "set" | "setcs" => self.state(c, w, env, out)?,
            "unset" => match self.set_kind(c.rest()) {
                SetKind::Display => out.push(opaque(ir::OpaqueClass::Inert, DISPLAY)),
                _ => out.push(opaque(ir::OpaqueClass::Private, OWN)),
            },
            // with no card a script save holds from where it stands (A28); with cards, before the
            // script's first analysis and with no reset in it, it is the deck's `.save` (Q6),
            // any other a state change the cards' runs would see (R49 (3))
            "save" => {
                let holds = if !self.cards {
                    ir::Applies::FromHere
                } else if !self.analysed && !self.resets {
                    ir::Applies::WholeRun
                } else {
                    self.changed = true;
                    out.push(opaque(ir::OpaqueClass::Unread, FORM));
                    return Ok(());
                };
                out.push(Op::Save {
                    rest: subst(c.rest(), env)?,
                    holds,
                    cmd: c.clone(),
                });
            }
            // ponytail: Unread until the outputs area's entry points for the exact case (Q5)
            "meas" => {
                let kind = c
                    .rest()
                    .split_whitespace()
                    .next()
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if self.meas_exact(&kind) {
                    out.push(Op::Meas {
                        rest: subst(c.rest(), env)?,
                        cmd: c.clone(),
                    });
                } else {
                    out.push(opaque(ir::OpaqueClass::Unread, FORM));
                }
            }
            // ponytail: `fourier` is the run area's `.four` on the current plot; Unread until a
            // scripted deck can write `Four` back (ng-run refuses it there)
            "fourier" => out.push(opaque(ir::OpaqueClass::Unread, FORM)),
            "osdi" => out.push(opaque(ir::OpaqueClass::InvalidInSource, NO_COMMAND)),
            _ => out.push(opaque(ir::OpaqueClass::InvalidInSource, NO_COMMAND)),
        }
        Ok(())
    }

    /// Whether a `meas` of `kind` here measures exactly the deck's one analysis of that kind
    /// (lead's Q5, "the cards a `run` stands for"): with no card of the kind, the script's one
    /// command of it, run before; with one card of it standing before the script, no command
    /// of it and the script's one `run`, run before. Both read the same once written (the card
    /// then a command at its place).
    fn meas_exact(&self, kind: &str) -> bool {
        let count = |list: &[String], w: &str| list.iter().filter(|x| *x == w).count();
        let cards: Vec<usize> = self
            .card_kinds
            .iter()
            .filter(|(k, _)| k == kind)
            .map(|&(_, at)| at)
            .collect();
        match cards.as_slice() {
            [] => count(&self.runs, kind) == 1 && self.seen.iter().any(|w| w == kind),
            [0] => {
                count(&self.runs, kind) == 0
                    && count(&self.runs, "run") == 1
                    && self.seen.iter().any(|w| w == "run")
            }
            _ => false,
        }
    }

    /// `option`, `set`: simulator options are state from here; display variables are inert.
    fn state(
        &mut self,
        c: &Cmd,
        w: &str,
        env: &[(String, String)],
        out: &mut Vec<Op>,
    ) -> Result<(), ir::Code> {
        let opaque = |class, code| Op::Opaque {
            class,
            code,
            cmd: c.clone(),
        };
        if c.rest().is_empty() {
            out.push(opaque(ir::OpaqueClass::Inert, DISPLAY));
            return Ok(());
        }
        let kind = self.set_kind(c.rest());
        let display_ok = matches!(w, "set" | "setcs");
        match kind {
            SetKind::Display if display_ok => out.push(opaque(ir::OpaqueClass::Inert, DISPLAY)),
            SetKind::Simulator => {
                self.changed = true;
                let keys = assignments(&subst(c.rest(), env)?);
                // a frontend variable survives `reset` (it is global, not the circuit's), while a
                // `Reset` restores every option: with a reset in the script it is ngspice's own
                let frontend = keys
                    .iter()
                    .any(|(k, _)| FRONTEND_KEYS.contains(&k.as_str()));
                if frontend && self.resets {
                    out.push(opaque(ir::OpaqueClass::Private, OWN));
                    return Ok(());
                }
                out.push(Op::State {
                    keys,
                    cmd: c.clone(),
                });
            }
            _ => out.push(opaque(ir::OpaqueClass::Private, OWN)),
        }
        Ok(())
    }

    /// Whether every name a `set`/`option` assigns is a display variable or a simulator option.
    fn set_kind(&self, rest: &str) -> SetKind {
        let keys = assignments(rest);
        if keys.is_empty() {
            return SetKind::Mixed;
        }
        let sim = |k: &str| {
            matches!(k, "temp" | "tnom" | "method") || read_run::KEYS.iter().any(|(n, _)| *n == k)
        };
        let shown =
            |k: &str| !sim(k) && !PRIVATE_VARS.contains(&k) && !CIRCUIT_OPTIONS.contains(&k);
        if keys.iter().all(|(k, _)| shown(k)) {
            SetKind::Display
        } else if keys.iter().all(|(k, _)| sim(k)) {
            SetKind::Simulator
        } else {
            SetKind::Mixed
        }
    }
}

/// The operations with every `alterparam` placeholder resolved: dropped where a `reset`
/// applied it, an `Inert` opaque where none did (S2).
fn settle(ops: Vec<Op>, applied: &[bool]) -> Vec<Op> {
    ops.into_iter()
        .filter_map(|op| match op {
            Op::Pending { id, cmd } => {
                (!applied.get(id).copied().unwrap_or(false)).then_some(Op::Opaque {
                    class: ir::OpaqueClass::Inert,
                    code: NO_RESET,
                    cmd,
                })
            }
            Op::Loop {
                var,
                target,
                values,
                body,
                cmd,
            } => Some(Op::Loop {
                var,
                target,
                values,
                body: settle(body, applied),
                cmd,
            }),
            other => Some(other),
        })
        .collect()
}

/// The loop law over a template pass: the first operation sets one target from the word, and
/// no other operation but a display command reads it; every word is a number. Returns the
/// target and the body after the setting.
fn law(template: &[Op], words: &[String], applied: &[bool]) -> Option<(Target, Vec<Op>)> {
    let waiting = |op: &Op| matches!(op, Op::Pending { id, .. } if !applied.get(*id).copied().unwrap_or(false));
    if template.iter().any(waiting) {
        return None;
    }
    let kept: Vec<Op> = settle(template.to_vec(), applied);
    // a parameter is set by `alterparam` and applied by the `reset` after it: the pass is
    // [Reset, Alter(param)], the loop's body keeps the Reset (it restores, a parameter stays)
    let (lead, kept) = match kept.as_slice() {
        [
            Op::Reset { .. },
            Op::Alter {
                target: Target::Param(_),
                ..
            },
            ..,
        ] => (Some(kept[0].clone()), kept[1..].to_vec()),
        _ => (None, kept),
    };
    let (first, rest) = kept.split_first()?;
    let Op::Alter { target, value, .. } = first else {
        return None;
    };
    if matches!(target, Target::Param(_)) != lead.is_some() {
        return None;
    }
    let rest: Vec<Op> = lead.into_iter().chain(rest.iter().cloned()).collect();
    if value != MARK || !words.iter().all(|w| is_number(w)) {
        return None;
    }
    rest.iter()
        .all(|op| !reads_mark(op))
        .then(|| (target.clone(), rest))
}

/// Whether an operation reads the template's marker outside a display command.
fn reads_mark(op: &Op) -> bool {
    match op {
        Op::Opaque { .. } | Op::Pending { .. } => false,
        Op::Loop { values, body, .. } => {
            values.iter().any(|v| v.contains(MARK)) || body.iter().any(reads_mark)
        }
        other => format!("{other:?}").contains(MARK),
    }
}

/// The analysis words of `nodes`, each `times` times (a loop body twice: it may run more).
fn analysis_words(nodes: &[Node], times: usize) -> Vec<String> {
    let mut out = Vec::new();
    for n in nodes {
        match n {
            Node::Cmd(c) if ANALYSES.contains(&c.word().as_str()) || c.word() == "run" => {
                out.extend(std::iter::repeat_n(c.word(), times));
            }
            Node::Foreach { body, .. } | Node::Repeat { body, .. } => {
                out.extend(analysis_words(body, times * 2));
            }
            _ => {}
        }
    }
    out
}

/// Whether any statement of the script holds a `reset`.
fn has_reset(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Cmd(c) => c.word() == "reset",
        Node::Foreach { body, .. } | Node::Repeat { body, .. } => has_reset(body),
        Node::Flow(_) => false,
    })
}
