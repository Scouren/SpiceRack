//! The store (D3: the store is source files): a deck's or a subcircuit's
//! ngspice source text behind a first line naming its dialect, and that
//! text read back. A subcircuit reads back into the cards its calls wrote;
//! a testbench's deck into its top's statements, as written. Text in
//! another dialect is first written as ngspice (`syntax::render`).
//!
//! Cites: ngspice manual §2.4 (`.subckt`), §2.6 (`.include`), §2.7
//! (`.lib`), §16 (`.control`, `pre_osdi`).
//!
//! Contents:
//! - `pub(super) const HEADER: &str`
//! - `pub(super) fn to_source(b: &Body) -> Result<String, String>`,
//!   `pub(super) fn definition(b: &Body) -> Result<String, String>`
//! - `pub(super) fn from_source(text: &str, d: Option<Lang>) -> Result<Body, String>`
//! - `pub(super) fn with_loads(text: &str, loads: &[String]) -> String`
//! - `pub(super) fn read_deck(text: &str, d: Option<Lang>) -> Result<Body, String>`
//! - `pub(super) fn library_marker(lib: &ModelLibrary) -> String`,
//!   `pub(super) fn libraries(text: &str) -> Vec<ModelLibrary>`

use crate::syntax::Lang;

use std::borrow::Cow;
use std::fmt::Write as _;

use super::model::{Body, Card, ModelLibrary, params};
use crate::syntax::ast::{Ast, Node, Tag};
use crate::syntax::token::{Tag as T, Tokens};
use crate::syntax::{parse, tokenize};

/// A stored text's first line, the name of the dialect after it following
/// (`* spicerack-source ngspice`): a comment in every SPICE dialect and in
/// Spectre.
pub(super) const HEADER: &str = "* spicerack-source ";

/// The start of a [`library_marker`] line.
const LIBRARY: &str = "* spicerack-library ";

/// `b` as the stored text of an ngspice deck that defines it
/// ([`with_loads`]): `.subckt` with its includes, `.lib` calls, models,
/// cards in the order the deck writes them, and its lines given as text.
///
/// # Errors
/// A `verilog()` block, which has no source form until it is lowered (a
/// run lowers it).
pub(super) fn to_source(b: &Body) -> Result<String, String> {
    let out = format!("{}\n{}\n.end\n", b.name, definition(b)?);
    Ok(with_loads(&out, &b.osdi))
}

/// `b` as the lines of its definition, `.subckt` through `.ends` (no
/// newline after it): what [`to_source`] stores, and what a definition
/// nested in another subcircuit is written as (`Subcircuit.subcircuit`;
/// ngspice-45 manual §2.6.1).
///
/// # Errors
/// As [`to_source`].
pub(super) fn definition(b: &Body) -> Result<String, String> {
    if !b.verilog.is_empty() {
        return Err(format!(
            "{}: a verilog() block has no source form; it is lowered when the deck is written",
            b.name
        ));
    }
    let ps = match b.params.is_empty() {
        true => String::new(),
        false => format!(" PARAMS:{}", params(&b.params)),
    };
    let ports: String = b.ports.iter().map(|p| format!(" {p}")).collect();
    let mut out = format!(".subckt {}{ports}{ps}\n", b.name);
    b.includes
        .iter()
        .for_each(|i| w(&mut out, format_args!(".include {i}\n")));
    b.libs
        .iter()
        .for_each(|(p, s)| w(&mut out, format_args!(".lib {p} {s}\n")));
    let cards = b
        .models
        .iter()
        .chain(b.elements.iter().chain(&b.instances).map(|c| &c.text));
    cards
        .chain(&b.raw)
        .for_each(|l| w(&mut out, format_args!("{l}\n")));
    w(&mut out, format_args!(".ends {}", b.name));
    Ok(out)
}

/// The subcircuit `text` defines: its first `.subckt`, and the loads at
/// its top. `d` is the text's dialect; `None` reads it off a [`HEADER`]
/// line, ngspice without one. A card keeps its text; one this API does not
/// write (a `.param`, an option) is kept as a line given as text.
///
/// # Errors
/// As [`read_deck`]; text that defines no subcircuit.
pub(super) fn from_source(text: &str, d: Option<Lang>) -> Result<Body, String> {
    let text = ngspice_text(text, d)?;
    let t = tokenize(&text, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let a = parse(&t, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let top = a.fields(a.root());
    let sc = top.iter().copied().find(|&n| a.tag(Node(n)) == Tag::Subckt);
    let Some(sc) = sc else {
        return Err("the text defines no subcircuit (.subckt)".into());
    };
    let mut b = head(&statement(&a, &t, sc));
    for &n in top {
        if a.tag(Node(n)) == Tag::Control {
            b.osdi.extend(loads(&a, &t, n));
        }
    }
    let kids = a.body(Node(sc));
    for (i, &n) in kids.iter().enumerate() {
        let line = statement(&a, &t, n);
        let first = line
            .split_whitespace()
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let rest = || line[first.len()..].trim().to_string();
        match first.as_str() {
            ".model" => b.models.push(line.clone()),
            ".include" | ".inc" => b.includes.push(rest()),
            ".lib" => match rest().rsplit_once(' ') {
                Some((p, s)) => b.libs.push((p.trim().into(), s.into())),
                None => b.raw.push(line.clone()),
            },
            w if a.tag(Node(n)) == Tag::Instance => {
                let card = Card::element(&first[1..], line.clone());
                match w.starts_with('x') {
                    true => b.instances.push(card),
                    false => b.elements.push(card),
                }
            }
            // A block nested in this one, a definition
            // (`Subcircuit.subcircuit`) or a conditional
            // (`Subcircuit.conditional`; its `.elseif` and `.else` are
            // chained from the `.if`, not statements of this body): its
            // lines through its `.ends` or `.endif`, which is where the
            // next statement, or this subcircuit's own `.ends`, starts.
            _ if matches!(a.tag(Node(n)), Tag::Subckt | Tag::If) => {
                let next = kids.get(i + 1).copied().map(Node);
                let to = next
                    .or_else(|| a.closer(Node(sc)))
                    .map_or(text.len(), |k| t.start(a.main_token(k)) as usize);
                let from = t.start(a.main_token(Node(n))) as usize;
                b.raw.push(text[from..to].trim_end().to_string());
            }
            _ => b.raw.push(line.clone()),
        }
    }
    Ok(b)
}

/// `text`, an ngspice deck's source (its title on the first line), stored:
/// [`HEADER`] `ngspice`, the title, then `loads` as a `.control` block of
/// `pre_osdi` lines (ngspice's form, which [`read_deck`] reads back as
/// loads), then the rest.
pub(super) fn with_loads(text: &str, loads: &[String]) -> String {
    let (title, rest) = text.split_once('\n').unwrap_or((text, ""));
    let mut out = format!("{HEADER}ngspice\n{title}\n");
    if !loads.is_empty() {
        out.push_str(".control\n");
        loads
            .iter()
            .for_each(|p| w(&mut out, format_args!("pre_osdi {p}\n")));
        out.push_str(".endc\n");
    }
    out.push_str(rest);
    out
}

/// The deck stored `text` holds, as a top block: its title the name
/// (`title`: without the comment leader a writer put before it), a
/// top-level `.control` block of `pre_osdi` lines alone its loads, and
/// every other statement but `.end` a line given as text, its bytes as
/// written (comments and blank lines after it included), so [`with_loads`]
/// of the deck written from it is `text` again. `d` as for [`from_source`].
///
/// # Errors
/// A dialect with no reader; text the renderer refuses to write as
/// ngspice; a [`HEADER`] naming no dialect.
pub(super) fn read_deck(text: &str, d: Option<Lang>) -> Result<Body, String> {
    // The title line as the deck's own dialect writes it: behind that
    // dialect's comment leader, or bare in the store's own text
    // (`with_loads`: a `HEADER` line first).
    let given = match text.starts_with(HEADER) {
        true => "",
        false => d.unwrap_or(Lang::NGSPICE).spelling().comment,
    };
    let text = ngspice_text(text, d)?;
    // Another dialect's text was written as ngspice just above: the title
    // is behind the ngspice writer's leader.
    let leader = match &text {
        Cow::Borrowed(_) => given,
        Cow::Owned(_) => Lang::NGSPICE.spelling().comment,
    };
    let t = tokenize(&text, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let a = parse(&t, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let top = a.fields(a.root());
    let line_start = |n: u32| {
        let at = t.start(a.main_token(Node(n))) as usize;
        text[..at].rfind('\n').map_or(0, |i| i + 1)
    };
    let starts: Vec<usize> = top.iter().map(|&n| line_start(n)).collect();
    assert!(starts.is_sorted(), "the top statements are in source order");
    let mut b = Body {
        name: title(&text, leader).to_string(),
        ..Body::default()
    };
    for (i, &n) in top.iter().enumerate() {
        let end = starts.get(i + 1).copied().unwrap_or(text.len());
        let span = &text[starts[i]..end];
        let kept = match a.tag(Node(n)) {
            Tag::End => continue,
            // What follows the title line, up to the first statement.
            Tag::Title => span.split_once('\n').map_or("", |(_, r)| r),
            // What follows the block's `.endc` line.
            Tag::Control if is_loads(&a, &t, n) => {
                b.osdi.extend(loads(&a, &t, n));
                let endc = span
                    .to_ascii_lowercase()
                    .find(".endc")
                    .unwrap_or(span.len());
                span[endc..].split_once('\n').map_or("", |(_, r)| r)
            }
            _ => span,
        };
        if !kept.is_empty() {
            b.raw
                .push(kept.strip_suffix('\n').unwrap_or(kept).to_string());
        }
    }
    Ok(b)
}

/// The comment line before a model library's ngspice line in the store:
/// `* spicerack-library <path> [corner=<c>] [setup=<p>]… [<family>=<p>]…`,
/// so a testbench read back writes each target's own path ([`libraries`]).
/// ponytail: paths are blank-separated words; a path with a blank in it
/// does not round-trip (an ngspice `.lib` path cannot hold one unquoted).
pub(super) fn library_marker(lib: &ModelLibrary) -> String {
    let mut out = format!("{LIBRARY}{}", lib.path);
    if let Some(c) = &lib.corner {
        w(&mut out, format_args!(" corner={c}"));
    }
    for s in &lib.setup_includes {
        w(&mut out, format_args!(" setup={s}"));
    }
    for (family, p) in &lib.backend_paths {
        w(&mut out, format_args!(" {family}={p}"));
    }
    out.push('\n');
    out
}

/// The model libraries `text`'s [`library_marker`] lines name, each
/// `placed`: its lines are already the deck's own, as written.
pub(super) fn libraries(text: &str) -> Vec<ModelLibrary> {
    let marked = text.lines().filter_map(|l| l.strip_prefix(LIBRARY));
    marked
        .map(|rest| {
            let mut words = rest.split_whitespace();
            let path = words.next().unwrap_or_default().to_string();
            let mut lib = ModelLibrary {
                name: std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("library")
                    .to_string(),
                path,
                corner: None,
                backend_paths: Default::default(),
                setup_includes: Vec::new(),
                placed: true,
            };
            for (k, v) in words.filter_map(|kv| kv.split_once('=')) {
                match k {
                    "corner" => lib.corner = Some(v.into()),
                    "setup" => lib.setup_includes.push(v.into()),
                    family => _ = lib.backend_paths.insert(family.into(), v.into()),
                }
            }
            lib
        })
        .collect()
}

// ---- private ----

fn w(s: &mut String, args: std::fmt::Arguments<'_>) {
    s.write_fmt(args).expect("a String takes any write");
}

/// A deck's title: line 1 of `text` without `leader`, the comment leader
/// its dialect's writers put before the title (`Spelling::comment`: `*`,
/// `//`, nothing for VACASK), so the deck written again has the same line.
/// One leader only: a writer adds one.
fn title<'a>(text: &'a str, leader: &str) -> &'a str {
    let line = text.lines().next().unwrap_or_default().trim();
    line.strip_prefix(leader).map_or(line, str::trim_start)
}

/// Statement `n`'s text: its first token to its last, continuation lines
/// joined as written.
fn statement(a: &Ast, t: &Tokens, n: u32) -> String {
    let first = a.main_token(Node(n));
    let eos = (first..t.len() as u32)
        .find(|&k| t.tags()[k as usize] == T::EndOfStatement)
        .expect("every statement ends");
    let last = (first..eos).last().unwrap_or(first);
    t.source()[t.start(first) as usize..t.end(last) as usize].to_string()
}

/// The `.subckt` line's name, ports and `PARAMS:` defaults (`k=v`).
fn head(line: &str) -> Body {
    let mut words = words(line).into_iter().skip(1);
    let name = words.next().unwrap_or_default();
    let mut b = Body {
        name,
        ..Body::default()
    };
    for w in words {
        if w.eq_ignore_ascii_case("params:") {
            continue;
        }
        match w.split_once('=') {
            Some((k, v)) => b.params.push((k.into(), v.into())),
            None => b.ports.push(w),
        }
    }
    b
}

/// `line` split at blanks outside brackets and quotes.
fn words(line: &str) -> Vec<String> {
    let (mut out, mut cur, mut depth, mut quote) = (Vec::new(), String::new(), 0i32, false);
    for c in line.chars() {
        match c {
            '\'' | '"' => quote = !quote,
            '(' | '{' | '[' if !quote => depth += 1,
            ')' | '}' | ']' if !quote => depth -= 1,
            c if c.is_whitespace() && depth == 0 && !quote => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
                continue;
            }
            _ => {}
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `text` as ngspice text: its [`HEADER`] line left out, read in `d` (or
/// the header's dialect, or ngspice), another dialect's written as ngspice
/// under its own title ([`title`]).
fn ngspice_text(text: &str, d: Option<Lang>) -> Result<Cow<'_, str>, String> {
    let (named, body) = match text.strip_prefix(HEADER) {
        Some(rest) => {
            let (name, body) = rest.split_once('\n').unwrap_or((rest, ""));
            (Some(super::def_::parse_lang(name.trim())?), body)
        }
        None => (None, text),
    };
    let d = d.or(named).unwrap_or(Lang::NGSPICE);
    if d.behaviours().text_family == Lang::NGSPICE {
        return Ok(Cow::Borrowed(body));
    }
    if d.lexicon().is_none() {
        return Err(format!("no {} reader", d.name()));
    }
    // The deck keeps its own title: the ngspice writer puts it on line 1,
    // where `read_deck` reads it.
    let name = title(body, d.spelling().comment);
    let w = crate::syntax::render::render_text(body, d, name, Lang::NGSPICE);
    Ok(Cow::Owned(w.map_err(|e| e.messages)?.text))
}

/// Whether `.control` block `n` holds `pre_osdi` lines and nothing else.
fn is_loads(a: &Ast, t: &Tokens, n: u32) -> bool {
    let body = a.body(Node(n));
    !body.is_empty() && loads(a, t, n).len() == body.len()
}

/// The paths a `.control` block's `pre_osdi` lines load.
fn loads(a: &Ast, t: &Tokens, n: u32) -> Vec<String> {
    let lines = a.body(Node(n)).iter().map(|&k| statement(a, t, k));
    let paths = lines.filter_map(|l| {
        let (cmd, path) = l.split_once(char::is_whitespace)?;
        cmd.eq_ignore_ascii_case("pre_osdi")
            .then(|| path.trim().to_string())
    });
    paths.collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Body {
        Body {
            name: "amp".into(),
            ports: vec!["in".into(), "out".into()],
            params: vec![("w".into(), "1u".into())],
            elements: vec![Card::element("1", "R1 in out {w*1k}".into())],
            instances: vec![Card::element("u1", "Xu1 out 0 load r=2k".into())],
            models: vec![".model dm D (is=1e-14)".into()],
            raw: vec![".options reltol=1e-6".into()],
            includes: vec!["/m.lib".into()],
            libs: vec![("/pdk.lib".into(), "tt".into())],
            osdi: vec!["/m.osdi".into()],
            ..Body::default()
        }
    }

    #[test]
    fn a_subcircuit_reads_back_as_the_same_cards() {
        // The store's oracle is its own text: read back and written again,
        // byte for byte (D3: the source is the lossless form). Checks: 3.
        let text = to_source(&sample()).unwrap();
        assert!(text.starts_with("* spicerack-source ngspice\namp\n.control\n"));
        let again = from_source(&text, None).unwrap();
        assert_eq!(to_source(&again).unwrap(), text);
        assert_eq!(again.instances[0].name, "u1");
    }

    #[test]
    fn p8_013_a_nested_definition_reads_back_whole() {
        // ngspice-45 manual §2.6.1: a definition may hold other definitions.
        // The store's oracle is its own text, read back and written again
        // byte for byte (D3). Checks: 3.
        let leaf = Body {
            name: "leaf".into(),
            ports: vec!["p".into()],
            elements: vec![Card::element("1", "R1 p 0 1k".into())],
            ..Body::default()
        };
        let mut outer = sample();
        outer.raw.push(definition(&leaf).unwrap());
        let text = to_source(&outer).unwrap();
        assert!(
            text.ends_with(".subckt leaf p\nR1 p 0 1k\n.ends leaf\n.ends amp\n.end\n"),
            "{text}"
        );
        let again = from_source(&text, None).unwrap();
        assert_eq!(
            again.raw.last().map(String::as_str),
            Some(".subckt leaf p\nR1 p 0 1k\n.ends leaf")
        );
        assert_eq!(to_source(&again).unwrap(), text);
    }

    #[test]
    fn a_deck_reads_back_as_its_statements_and_loads() {
        // ngspice manual §16 (`pre_osdi` in `.control`): the loads block
        // comes back as loads, every other statement as written, the blank
        // line before `.subckt` kept. Checks: 4.
        let deck = "tb\n\n.global vdd\n\n.subckt c a\nR1 a 0 1k\n.ends c\nX1 n c\nsimulator lang=spectre\nhb1 hb fundfreqs=[1G] maxharms=[5]\nsimulator lang=spice\n.end\n";
        let text = with_loads(deck, &["/m.osdi".into()]);
        let b = read_deck(&text, None).unwrap();
        assert_eq!(b.name, "tb");
        assert_eq!(b.osdi, ["/m.osdi"]);
        assert_eq!(
            b.raw.len(),
            5,
            "the blank line after the loads: {:?}",
            b.raw
        );
        let raw: String = b.raw.iter().map(|l| format!("{l}\n")).collect();
        assert_eq!(with_loads(&format!("tb\n{raw}.end\n"), &b.osdi), text);
    }

    #[test]
    fn p8_312_a_written_decks_title_reads_back_without_its_comment_leader() {
        // ngspice manual §2.1 (line 1 is the title, whatever it holds); the
        // ngspice writer puts `* ` before the name it is given
        // (`render/spice/mod.rs`: `"{comment} {name}"`), so the name a
        // written deck is read under is line 1 without that leader, and
        // writing it again gives the same line (ROUNDTRIP-TRIAGE K1 b).
        // Checks: 4.
        let name = |text: &str| read_deck(text, None).unwrap().name;
        assert_eq!(name("* rc\nR1 a 0 1k\n.end\n"), "rc");
        // One leader only: the writer adds one.
        assert_eq!(name("* * rc\nR1 a 0 1k\n.end\n"), "* rc");
        // A bare title is the title.
        assert_eq!(name("rc\nR1 a 0 1k\n.end\n"), "rc");
        // The store's own text holds the title bare: a leading `*` is the
        // title's own (`with_loads` wrote the name as it was).
        let stored = with_loads("* rc\nR1 a 0 1k\n.end\n", &[]);
        assert_eq!(name(&stored), "* rc");
    }

    #[test]
    fn p8_316_a_foreign_deck_is_read_under_its_own_title() {
        // Spectre UG 5.1.41 ("The first line is taken to be the title");
        // ngspice manual §2.1 and HSPICE the same; VACASK's first line is
        // the title, bare (`render/named/vacask.rs` writes it so). The
        // three decks are what each writer gives a one-resistor `amp` with
        // an operating point (ROUNDTRIP-TRIAGE K1 a). Checks: 3.
        let name = |text: &str, d: Lang| read_deck(text, Some(d)).unwrap().name;
        let hspice = "* amp\nR1 a 0 1k\n.op\n.end\n";
        assert_eq!(name(hspice, Lang::HSPICE), "amp");
        let spectre = "// amp\n\nsimulator lang=spectre\n\n\nr1 (a 0) resistor r=1k\n\nop1 dc\n";
        assert_eq!(name(spectre, Lang::SPECTRE), "amp");
        let vacask = "amp\n\nground 0\n\nload \"spice/resistor.osdi\"\n\nmodel resistor sp_resistor\n\nr1 (a 0) resistor r=1e3\n\ncontrol\n  options rawfile=\"binary\"\n  analysis op1 op\nendc\n";
        assert_eq!(name(vacask, Lang::VACASK), "amp");
    }

    #[test]
    fn a_header_names_the_dialect_and_text_needs_a_subcircuit() {
        assert!(from_source("t\nR1 a 0 1k\n.end\n", None).is_err());
        assert!(
            from_source("t\n", Some(Lang::CDL)).is_err(),
            "no CDL reader"
        );
        assert!(from_source("* spicerack-source nonesuch\nt\n", None).is_err());
        // Spectre text is written as ngspice first (Spectre UG p.94); the
        // header names it, and an argument overrides the header.
        let spectre = "// t\nsubckt cell (a b)\nr1 (a b) resistor r=1k\nends cell\n";
        for (text, d) in [
            (format!("{HEADER}spectre\n{spectre}"), None),
            (format!("{HEADER}ngspice\n{spectre}"), Some(Lang::SPECTRE)),
        ] {
            let b = from_source(&text, d).unwrap();
            assert_eq!(
                (b.name.as_str(), b.ports.len(), b.elements.len()),
                ("cell", 2, 1)
            );
        }
    }
}
