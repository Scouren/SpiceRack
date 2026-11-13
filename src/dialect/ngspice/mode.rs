//! A deck's start-up file (`.spiceinit`, `Sources::environment`, R87) → the statements whose
//! reading it changes, each kept as its own text, `Unread`, with a code naming the cause: a
//! compatibility mode (`set ngbehavior=…`: `ps` PSpice, `lt` LTspice, `a` the whole deck rather
//! than its included files only) or another start-up command (`set stepsizelimit`). No IR rule
//! reads a mode yet (R87: "no rule yet"); a statement no trigger names reads as plain ngspice.
//!
//! The triggers are ngspice-45's own rewrites, each a row of the inventory below (`arg` of the
//! code), tested on the statement's text and kept wide: a statement a mode might change is
//! refused, never read as plain (a missed rewrite is a wrong number with no message).
//!
//! Cites: ngspice-45 (source of `/nix/store/jfngv2gd…-ngspice-45`, tarball
//! `b9hi97vj…-ngspice-45.tar.gz`): `frontend/inpcompat.c` (`set_compat_mode` 60-110: each flag by
//! `strstr`; `pspice_compat` 662-1180, `ltspice_compat` 1735-1914), `frontend/inpcom.c` (1098-1100
//! the whole deck under `a`, 1617-1620 each included file without it; 1126 `rem_mfg_from_models`;
//! 1450-1459 `.lib` read as `.inc`; 3544 `$`; 5195 `d` terminals), `spicelib/parser/inp2r.c:82`,
//! `inp2c.c:74`, `inp2l.c:73` (RKM values), `ptfuncs.c` (69-139 powers, 275-283 `exp`),
//! `numparam/xpressn.c:779-805`, `numparam/spicenum.c:186`; runs: settle `spiceinit/`, the 41
//! corpus decks under a mode (21 fail without it, `soa/nic_soa.sp` changes numbers).
//!
//! Contents: `pub(super) fn lower(lw: &mut Lower<'_, '_>, st: Stmt, next: fn(&mut Lower<'_, '_>, Stmt))`

use super::codes;
use super::cst;
use super::read::{Lower, Stmt};
use crate::ir;
use crate::syntax::ast::Tag;

/// What a start-up file sets: the mode flags (`strstr` on the value, as ngspice) and
/// `stepsizelimit`.
#[derive(Default)]
struct Env {
    ps: bool,
    lt: bool,
    a: bool,
    stepsizelimit: bool,
}

/// Lowers `st` by `next` (the read spine's dispatch), unless the deck's start-up file changes its
/// reading: then an `Unread` opaque of its text with [`codes::MODE_PS`], [`codes::MODE_LT`] or
/// [`codes::START_UP`], `arg` the inventory row.
pub(super) fn lower(lw: &mut Lower<'_, '_>, st: Stmt, next: fn(&mut Lower<'_, '_>, Stmt)) {
    if !refused(lw, st) {
        next(lw, st);
    }
}

// ---- private ----

impl Env {
    /// The last `set ngbehavior=` and any `set stepsizelimit` of `text`.
    fn read(text: &str) -> Env {
        let mut env = Env::default();
        for line in text.lines().map(|l| l.trim().to_ascii_lowercase()) {
            let Some(rest) = line.strip_prefix("set ") else {
                continue;
            };
            let rest = rest.trim();
            if rest == "stepsizelimit" {
                env.stepsizelimit = true;
            }
            if let Some(v) = rest.strip_prefix("ngbehavior").map(str::trim) {
                let v = v.trim_start_matches('=').trim();
                env.ps = v.contains("ps");
                env.lt = v.contains("lt");
                env.a = v.contains('a');
            }
        }
        env
    }
}

/// Whether the start-up file changes `st`'s reading; if so, its opaque is pushed.
fn refused(lw: &mut Lower<'_, '_>, st: Stmt) -> bool {
    let Some((_, init)) = lw.src.environment() else {
        return false;
    };
    let tag = lw.ast.tag(st.node);
    if matches!(tag, Tag::Title | Tag::End) {
        return false;
    }
    let env = Env::read(init);
    // the source lines the statement stands on, comments included (a `$` comment, row 4, is
    // text to the mode: the opaque keeps it)
    let src = lw.toks.source();
    let (a, b) = cst::extent(lw.ast, lw.toks, st.node);
    let from = src[..lw.toks.start(a) as usize]
        .rfind('\n')
        .map_or(0, |k| k + 1);
    let to = lw.toks.end(b) as usize;
    let to = src[to..].find('\n').map_or(src.len(), |k| to + k);
    let included = lw.role != ir::Role::Deck;
    let text = &src[from..to];
    // a block's own card is its first line and its `+` lines; its body is read statement by
    // statement (a refused head keeps the block as one opaque, as any refused block)
    let block = matches!(tag, Tag::Subckt | Tag::Section | Tag::If);
    let head = if block { head_lines(text) } else { text };
    let Some((code, row)) = trigger(&env, included, tag == Tag::Control, head) else {
        return false;
    };
    let span = lw.span(st.node);
    lw.b.opaque(st.scope, "ngspice", ir::OpaqueClass::Unread, text, span);
    lw.diags.push(code, span, row);
    true
}

/// The code and inventory row of the first trigger that names `text`, if one does.
fn trigger(env: &Env, included: bool, control: bool, text: &str) -> Option<(ir::Code, u32)> {
    let joined = text.replace("\n+", " ").to_ascii_lowercase();
    let line = joined.trim();
    let words: Vec<&str> = line.split_whitespace().collect();
    let first = words.first().copied().unwrap_or("");
    let card = first.as_bytes().first().copied().unwrap_or(b'*');
    let model = first == ".model";
    let has = |w: &str| words.contains(&w);
    if env.stepsizelimit && (first == ".tran" || control && has("tran")) {
        return Some((codes::START_UP, 1)); // `set stepsizelimit`: tran's step is at most tstep
    }
    let ps = |row: u32| Some((codes::MODE_PS, row));
    let lt = |row: u32| Some((codes::MODE_LT, row));
    if (env.ps || env.lt) && first.starts_with(".lib") {
        return if env.ps { ps(1) } else { lt(1) }; // read as `.inc` of the whole file
    }
    if (env.ps || env.lt)
        && model
        && ["mfg=", "icrating=", "vceo=", "type="]
            .iter()
            .any(|k| line.contains(k))
    {
        return if env.ps { ps(5) } else { lt(5) };
    }
    if env.ps && !control && dollar(line) {
        return ps(4); // `$` is no comment
    }
    if env.ps && card == b'd' && !has("thermal") {
        // plain: the model is the item before the first `off`, `thermal` or `k=v` from the
        // fourth on (semi.rs); ps: the third item (two nodes)
        let stop = words
            .iter()
            .skip(3)
            .position(|w| *w == "off" || w.contains('='))
            .map_or(words.len(), |k| k + 3);
        if stop != 4 {
            return ps(7);
        }
    }
    let behavioural = matches!(card, b'b' | b'e' | b'g' | b'r' | b'c' | b'l');
    if env.ps && behavioural && (call(line, "exp") || call(line, "pwr")) {
        return ps(14); // `exp` linear above 14, `pwr(0, b<0)` fudged
    }
    if env.ps && card == b'x' && subckt_name(&words).is_some_and(|n| n.contains('-')) {
        return ps(17); // numparam's subcircuit name may hold a `-`
    }
    if env.lt && matches!(card, b'r' | b'c' | b'l') && !rkm_plain(card, words.get(3)) {
        return lt(9); // the value read by RKM rules
    }
    if env.lt && (line.contains('^') || line.contains("**") || call(line, "pow")) {
        return lt(10); // a power of a negative base
    }
    let p = env.ps && (env.a || included);
    let l = env.lt && (env.a || included);
    if l {
        if ["uplim", "dnlim", "uplim_tanh", "dnlim_tanh"]
            .iter()
            .any(|f| call(line, f))
        {
            return lt(46);
        }
        if first == ".backanno" || card == b'r' && has("noiseless") {
            return lt(47);
        }
        let diode = words
            .get(2)
            .is_some_and(|t| *t == "d" || t.starts_with("d("));
        if model
            && diode
            && [
                "roff",
                "ron",
                "rrev",
                "vfwd",
                "vrev",
                "revepsilon",
                "epsilon",
                "revilimit",
                "ilimit",
            ]
            .iter()
            .any(|k| key(line, k))
        {
            return lt(49); // an LTspice ideal diode, read as XSPICE `sidiode`
        }
    }
    if !p {
        return None;
    }
    let pspice = [
        (model && line.contains("ako:"), 19),
        (first == ".distribution", 20),
        (matches!(card, b'e' | b'g') && line.contains("table("), 21),
        (line.contains("{{") || nested_brace(line), 22),
        (
            ["temp", "vt", "gmin"]
                .iter()
                .any(|w| ident_after_value(line, w)),
            29,
        ),
        (
            ["limit", "pwr", "pwrs", "stp", "if", "int"]
                .iter()
                .any(|f| call(line, f)),
            30,
        ),
        (card == b'u', 24),
        (
            (first == ".subckt" || card == b'x') && line.contains("params:") && line.contains(','),
            25,
        ),
        (model && line.contains("level") && pspice_level(line), 26),
        (model && line.contains("lpnp"), 28),
        (
            model && (line.contains("dev=") || line.contains("lot=")),
            31,
        ),
        (
            matches!(card, b'r' | b'c' | b'l') && (words.len() < 4 || has("tc")),
            33,
        ),
        (first.starts_with("*#"), 34),
        (!control && (line.contains('&') || line.contains('|')), 35),
        (!control && line.contains('~') && card != b'a', 37),
        (
            model
                && ["t_abs", "t_rel_global", "t_measured"]
                    .iter()
                    .any(|k| line.contains(k)),
            38,
        ),
        (
            card == b'q' && (line.contains('[') || line.contains('{')),
            39,
        ),
        (
            card == b'd' && words.get(4).is_some_and(|w| number(w) > 0.0),
            40,
        ),
        (
            model && (line.contains("vswitch") || line.contains("iswitch")),
            41,
        ),
    ];
    pspice
        .iter()
        .find(|(hit, _)| *hit)
        .and_then(|&(_, row)| ps(row))
}

/// The first line of `text` and the `+` lines that continue it.
fn head_lines(text: &str) -> &str {
    let mut end = text.find('\n').unwrap_or(text.len());
    // Bounded: each turn moves `end` past one more line.
    while end < text.len() && text[end + 1..].trim_start().starts_with('+') {
        end = text[end + 1..]
            .find('\n')
            .map_or(text.len(), |k| end + 1 + k);
    }
    &text[..end]
}

/// The subcircuit an X card names: its last bare word before `params:` or the first `k=v`.
fn subckt_name<'w>(words: &[&'w str]) -> Option<&'w str> {
    let bare = words
        .iter()
        .take_while(|w| **w != "params:" && !w.contains('='))
        .count();
    words
        .get(bare.checked_sub(1)?)
        .copied()
        .filter(|_| bare > 1)
}

/// Whether `name` stands in `line` as a parameter key: `name=` or `name =`, not inside a word.
fn key(line: &str, name: &str) -> bool {
    line.match_indices(name).any(|(at, _)| {
        let before = line[..at]
            .bytes()
            .last()
            .is_none_or(|c| !(c.is_ascii_alphanumeric() || c == b'_'));
        before && line[at + name.len()..].trim_start().starts_with('=')
    })
}

/// Whether a `$` starts a comment in plain ngspice (after a blank, a tab or a comma).
fn dollar(line: &str) -> bool {
    line.as_bytes()
        .windows(2)
        .any(|w| matches!(w[0], b' ' | b'\t' | b',') && w[1] == b'$')
}

/// Whether `name` stands in `line` as a call: no identifier character before it, `(` after it,
/// blanks between allowed (run: `ddt-test2.sp`'s `.func border(x) if (…)` is expanded by the
/// mode, "no such function 'if'" without it).
fn call(line: &str, name: &str) -> bool {
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    line.match_indices(name).any(|(at, _)| {
        let before = line[..at].bytes().last().is_none_or(|c| !ident(c));
        before && line[at + name.len()..].trim_start().starts_with('(')
    })
}

/// Whether the identifier `name` stands in `line` after its first `=` or `{` (inventory row 29).
fn ident_after_value(line: &str, name: &str) -> bool {
    let Some(from) = line.find(['=', '{']) else {
        return false;
    };
    let tail = &line[from..];
    let ident = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    tail.match_indices(name).any(|(at, _)| {
        let before = tail[..at].bytes().last().is_none_or(|c| !ident(c));
        let after = tail[at + name.len()..]
            .bytes()
            .next()
            .is_none_or(|c| !ident(c));
        before && after
    })
}

/// Whether a `{` opens inside another `{` (inventory row 22).
fn nested_brace(line: &str) -> bool {
    let mut depth = 0u32;
    for c in line.bytes() {
        match c {
            b'{' if depth >= 1 => return true,
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
    }
    false
}

/// Whether a MOS `level=` is one PSpice mode rewrites (5, 6, 7, 8) or a BJT `level=2`.
fn pspice_level(line: &str) -> bool {
    let squeezed: String = line.split_whitespace().collect();
    let mos = squeezed.contains("nmos") || squeezed.contains("pmos");
    let bjt = squeezed.contains("npn") || squeezed.contains("pnp");
    let Some(at) = squeezed.find("level=") else {
        return false;
    };
    let digits: String = squeezed[at + 6..]
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    match digits.parse::<u32>() {
        Ok(5..=8) => mos,
        Ok(2) => bjt,
        _ => false,
    }
}

/// Whether the R, C or L value token reads the same by RKM rules as by plain ngspice: a plain
/// number with at most one plain scale (no trailing letters, no `r` decimal point, no digits
/// after the scale, and no `f` or `a` on an R); a `{…}` or a missing token is no RKM read.
fn rkm_plain(card: u8, token: Option<&&str>) -> bool {
    let Some(t) = token else {
        return true;
    };
    if t.starts_with(['{', '\'']) || t.contains('=') {
        return true;
    }
    let body = t.trim_start_matches(['+', '-']);
    let digits = body
        .find(|c: char| !(c.is_ascii_digit() || c == '.'))
        .unwrap_or(body.len());
    if digits == 0 {
        return false;
    }
    let mut rest = &body[digits..];
    if let Some(e) = rest.strip_prefix('e') {
        let e = e.trim_start_matches(['+', '-']);
        let n = e.find(|c: char| !c.is_ascii_digit()).unwrap_or(e.len());
        if n == 0 {
            return false;
        }
        rest = &e[n..];
    }
    let scales: &[&str] = if card == b'r' {
        &["", "t", "g", "meg", "k", "m", "u", "n", "p"]
    } else {
        &["", "t", "g", "meg", "k", "m", "u", "n", "p", "f", "a"]
    };
    scales.contains(&rest)
}

/// A leading number of `word` as C `atof` reads it (0 when none).
fn number(word: &str) -> f64 {
    let end = word
        .find(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | '+' | '-' | 'e')))
        .unwrap_or(word.len());
    word[..end].parse().unwrap_or(0.0)
}
