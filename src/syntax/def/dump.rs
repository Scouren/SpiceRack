//! A registered row → canonical text (`Lang::dump`): one line per field
//! of its lexicon, spelling, cells, behaviours, codes, maps and hook heads,
//! a language written by name, never by handle. Two definitions that
//! freeze equal rows dump the same bytes: what an equal re-registration
//! and the Python binding's parity test compare.
//!
//! Cites: none.
//!
//! Contents:
//! - `pub(crate) fn dump(r: &Row) -> String`

use std::fmt::Write as _;

use crate::syntax::lang::{CONSTRUCTS, Row};
use crate::syntax::lexicon::Lexicon;
use crate::syntax::render::capability::{Action, Alt};

/// `r` as canonical text.
pub(crate) fn dump(r: &Row) -> String {
    let mut o = String::new();
    let mut line = |k: &str, v: String| writeln!(o, "{k} {v}").expect("a String takes writes");
    line("name", r.name.into());
    line("base", r.base.map_or("-".into(), |b| b.name().into()));
    line("runs_on", format!("{:?}", r.runs_on));
    match r.lexicon {
        Some(l) => lexicon(l, &mut line),
        None => line("lexicon", "-".into()),
    }
    let sp = r.spelling;
    line("sp.lexicon", sp.lexicon.lang.name().into());
    line("sp.comment", format!("{:?}", sp.comment));
    line("sp.path_quote", format!("{:?}", sp.path_quote));
    line("sp.expr", format!("{:?}", sp.expr));
    line("sp.func", format!("{:?}", sp.func));
    line("sp.subckt_params", format!("{:?}", sp.subckt_params));
    line("sp.ngspice_numbers", format!("{:?}", sp.ngspice_numbers));
    for (i, c) in CONSTRUCTS.into_iter().enumerate() {
        let a = r.cells[i];
        let v = match plugin(a) {
            true => format!(
                "{a:?} {}",
                r.codes[usize::from(r.details[i] - r.first_code)].0
            ),
            false => format!("{a:?}"),
        };
        line(&format!("cell.{c:?}"), v);
    }
    let b = r.behaviours;
    let family = format!("text_family: {:?}", b.text_family);
    let named = format!("text_family: {}", b.text_family.name());
    line("behaviours", format!("{b:?}").replace(&family, &named));
    for c in r.codes {
        line("code", format!("{} {:?} {:?}", c.0, c.1, c.2));
    }
    for h in r.heads {
        line("write_head", format!("{:?} {:?}", h.0, h.1));
    }
    for m in r.write_masters {
        line("write_master", format!("{:?} {:?}", m.0, m.1));
    }
    for m in r.options {
        line("option", format!("{:?} {:?}", m.0, m.1));
    }
    for h in r.hooks {
        line("hook", format!("{:?}", h.0));
    }
    o
}

// ---- private ----

/// A cell that raises a definition's own code.
fn plugin(a: Action) -> bool {
    use crate::syntax::diag::Code;
    matches!(
        a,
        Action::Refuse(Code::W0201) | Action::Spell(Alt::Omit(Code::W0202))
    )
}

/// `l`'s fields, one line each.
fn lexicon(l: &Lexicon, line: &mut impl FnMut(&str, String)) {
    line("lex.lang", l.lang.name().into());
    line("lex.title", format!("{:?}", l.title));
    line("lex.fold_case", format!("{:?}", l.fold_case));
    line("lex.comment_lines", format!("{:?}", l.comment_lines));
    line("lex.comment_col1", format!("{:?}", l.comment_col1));
    line(
        "lex.blank_led_comment",
        format!("{:?}", l.blank_led_comment),
    );
    line("lex.cont_skip", format!("{:?}", l.cont_skip));
    line("lex.comment_tails", format!("{:?}", l.comment_tails));
    line("lex.block_comment", format!("{:?}", l.block_comment));
    line("lex.long_strings", format!("{:?}", l.long_strings));
    line("lex.dollar", format!("{:?}", l.dollar));
    line("lex.plus", format!("{:?}", l.plus));
    line("lex.backslash", format!("{:?}", l.backslash));
    line("lex.end", format!("{:?}", l.end));
    line("lex.stop", format!("{:?}", l.stop));
    line("lex.join", format!("{:?}", l.join));
    line("lex.quote", format!("{:?}", l.quote));
    line("lex.string", format!("{:?}", l.string));
    line("lex.string_expr", format!("{:?}", l.string_expr));
    line("lex.name_escape", format!("{:?}", l.name_escape));
    line("lex.ident_start", format!("{:?}", l.ident_start));
    line("lex.ident_continue", format!("{:?}", l.ident_continue));
    line("lex.directive", format!("{:?}", l.directive));
    line("lex.head_words", format!("{:?}", l.head_words));
    line("lex.heads_after", format!("{:?}", l.heads_after));
    line("lex.operators", format!("{:?}", l.operators));
    line("lex.expr_braces", format!("{:?}", l.expr_braces));
    line("lex.caret", format!("{:?}", l.caret));
    line("lex.infix", format!("{:?}", l.ops.infix));
    line("lex.right", format!("{:?}", l.ops.right));
    line("lex.prefix", format!("{:?}", l.ops.prefix));
    line("lex.prefix_bp", format!("{:?}", l.ops.prefix_bp));
    line("lex.probes", format!("{:?}", l.ops.probes));
    line("lex.index", format!("{:?}", l.ops.index));
    line("lex.lib_sections", format!("{:?}", l.lib_sections));
    line("lex.exponent", format!("{:?}", l.numbers.exponent));
    line("lex.scales", format!("{:?}", l.numbers.scales));
    line("lex.tail", format!("{:?}", l.numbers.tail));
    line("lex.arithmetic", format!("{:?}", l.numbers.arithmetic));
    line("lex.keywords", format!("{:?}", l.keywords));
    line("lex.named", format!("{:?}", l.named));
    line("lex.devices", format!("{:?}", l.devices));
    line("lex.masters", format!("{:?}", l.masters));
    line("lex.analyses", format!("{:?}", l.analyses));
    let regions: Vec<_> = l.regions.iter().map(|r| (r.0, r.1.lang.name())).collect();
    line("lex.regions", format!("{regions:?}"));
    line(
        "lex.regions_registered",
        format!("{:?}", l.regions_registered),
    );
}
