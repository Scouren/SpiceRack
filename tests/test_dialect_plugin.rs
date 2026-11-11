//! Dialect plugins (`syntax::def`, `DIALECT-PLUGIN.md` §2, §5, §6.2-§6.5):
//! acme, the doc's small dialect, declared in Rust from public items only,
//! read and written; its own refusal codes, a write hook (Rust and
//! external), extensions of a built-in and of a plugin, every
//! registration rule, determinism, a plugin region in a given deck.
//!
//! Oracles, named in each test: the doc's §2 spec of acme (what each call
//! writes); a round trip (`write(read(A1, acme), acme) == A1`); a
//! metamorphic relation (acme text and the ngspice text it spells read to
//! the same ngspice deck; an extension changing no written row writes its
//! base's bytes; two registration orders, 1 and 8 threads, the same
//! bytes); the catalogue (`W0201` with the definition's id and message);
//! the pinned dumps `tests/fixtures/dialects/*.dump`, the Python binding's
//! oracle too (`SPICERACK_BLESS=1` rewrites them).
//!
//! Contents: none (tests only).

#[path = "common/acme.rs"]
mod acme;

use std::sync::atomic::Ordering;

use acme::{NOISE_CALLS, NOISE_CARDS, acme_as};
use spicerack::syntax::behaviour::DotRule;
use spicerack::syntax::def::{Answer, DialectDef, RegisterError, RunsOn, register};
use spicerack::syntax::diag::{Code, Kind};
use spicerack::syntax::lexicon::Scale;
use spicerack::syntax::render::capability::Construct;
use spicerack::syntax::render::{Diags, Numbers, Source, render_pending, render_text, splice};
use spicerack::syntax::token::Tag;
use spicerack::syntax::{Dialect, Lang, parse, tokenize};

/// A deck exercising every acme row but the hook.
const DECK: &str = "\
* t
.param rv=2k
.func f(x) {x*2}
.subckt load n PARAMS: r=1k
R1 n 0 {r*f(1)}
.ends load
V1 in 0 1 AC 1
R1 in mid {rv}
C1 mid 0 q={1p*v(mid)}
X1 mid load r=2k
.options reltol=1e-4 abstol=1e-12
.tran 1n 1u
.end
";

/// The round trip's decks: [`DECK`], and small ngspice decks with models,
/// sources, nested subcircuits, measures and sweeps.
const FIXTURES: &[&str] = &[
    DECK,
    "* rc\nV1 in 0 PULSE(0 1 0 1n 1n 5u 10u)\nR1 in out 1k\nC1 out 0 1n\n.tran 10n 20u\n.meas tran vmax MAX v(out)\n.end\n",
    "* diode\n.model dx D(is=1e-14 n=1.05)\nV1 a 0 DC 0\nD1 a 0 dx\n.dc V1 0 0.8 0.01\n.end\n",
    "* nest\n.subckt inner a b\nR1 a b 1k\n.ends inner\n.subckt outer a b\nX1 a m inner\nX2 m b inner\n.ends outer\nV1 in 0 1\nX1 in 0 outer\n.op\n.end\n",
    "* ac\n.param g=2\nV1 in 0 AC 1\nE1 out 0 in 0 2\nR2 in 0 {g*1k}\nR1 out 0 1meg\n.ac dec 10 1 1g\n.end\n",
];

// ---- private ----

fn acme() -> DialectDef {
    acme_as("acme")
}

fn acme_lang() -> Lang {
    register(acme()).expect("acme registers")
}

/// §2's `MyHspice` under `name`.
fn my_hspice_as(name: &str) -> DialectDef {
    DialectDef::new(name)
        .extends(Lang::HSPICE)
        .comment_tail("//")
        .code("MY001", Kind::Refusal, "our HSPICE build has no .step")
        .refuse_code(Construct::Step, "MY001")
}

/// `deck` read in `from`, written for `to`, or the refusal's messages.
fn write(deck: &str, from: Lang, to: Lang) -> Result<String, String> {
    render_text(deck, from, "t", to)
        .map(|w| w.text)
        .map_err(|e| e.messages)
}

/// `deck`'s refusal in `to`.
fn refusal(deck: &str, from: Lang, to: Lang) -> Diags {
    render_text(deck, from, "t", to).expect_err("refused")
}

/// Oracle: §2's spec of acme, each call's text in the written deck.
#[test]
fn acme_writes_its_own_rows() {
    let acme = acme_lang();
    let a = write(DECK, Lang::NGSPICE, acme).expect("written");
    for line in [
        ".subcircuit load n PARAMS: r=1k",
        ".endsub load",
        ".param f(x)='x*2'",
        "C1 mid 0 Q={1p*v(mid)}",
        ".options rtol=1e-4 abstol=1e-12",
    ] {
        assert!(a.lines().any(|l| l == line), "{line:?} in\n{a}");
    }
    assert!(
        !a.contains(".subckt") && !a.contains(".ends ") && !a.contains("reltol"),
        "{a}"
    );
}

/// Oracle: metamorphic: acme text (`#` comments, `;` tails,
/// `.subcircuit`, `x`) reads to the ngspice deck the ngspice text it
/// spells reads to.
#[test]
fn acme_text_reads_as_its_ngspice_spelling() {
    let acme = acme_lang();
    let own = "# t\n# a note\n.subcircuit load n PARAMS: r=1k ; a tail\nR1 n 0 {r}\n.endsub load\nV1 in 0 2x\nX1 in load r=2k\n.op\n.end\n";
    let ng = "* t\n* a note\n.subckt load n PARAMS: r=1k\nR1 n 0 {r}\n.ends load\nV1 in 0 2meg\nX1 in load r=2k\n.op\n.end\n";
    assert_eq!(
        write(own, acme, Lang::NGSPICE),
        write(ng, Lang::NGSPICE, Lang::NGSPICE)
    );
}

/// Oracle: round trip: each fixture written to acme reads back in acme to
/// the same acme deck, byte for byte.
#[test]
fn acme_round_trip_is_a_fixpoint() {
    let acme = acme_lang();
    for d in FIXTURES {
        let a1 = write(d, Lang::NGSPICE, acme).unwrap_or_else(|e| panic!("{d}\n{e}"));
        let a2 = write(&a1, acme, acme).expect("rewritten");
        assert_eq!(a2, a1, "deck:\n{d}");
    }
}

/// Oracle: the catalogue: a Laplace source refused `W0201` with acme's
/// own id and message at its line.
#[test]
fn acme_refuses_laplace_with_its_own_code() {
    let acme = acme_lang();
    let deck = "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s*1e-3))\nR1 out 0 1k\n.op\n.end\n";
    let e = refusal(deck, Lang::NGSPICE, acme);
    let codes: Vec<Code> = e.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0201]);
    assert_eq!(
        e.messages,
        "t:3:1: W0201 acme:ACME001 acme has no Laplace sources\n"
    );
    // A cell raising a definition's second code names that one.
    let second = DialectDef::new("lap-second")
        .extends(Lang::NGSPICE)
        .code("L1", Kind::Refusal, "the first")
        .code("L2", Kind::Refusal, "the second")
        .refuse_code(Construct::Laplace, "L2");
    let second = register(second).expect("registers");
    let e = refusal(deck, Lang::NGSPICE, second);
    assert_eq!(e.messages, "t:3:1: W0201 lap-second:L2 the second\n");
}

/// Oracle: §6.3: the hook is called once a deck with every `.noise` card
/// in output order, its texts spliced in; a card it refuses refuses the
/// deck with `ACME002` at the `.noise` card's source line.
#[test]
fn acme_noise_hook() {
    let acme = acme_lang();
    let deck = "* t\nV1 in 0 1 AC 1\nR1 in out 1k\nR2 out 0 1k\n.noise v(out) V1 dec 10 1 1meg\n.noise v(in) V1 dec 5 1 1k\n.end\n";
    let calls = NOISE_CALLS.load(Ordering::SeqCst);
    let cards = NOISE_CARDS.load(Ordering::SeqCst);
    let a = write(deck, Lang::NGSPICE, acme).expect("written");
    let noise: Vec<&str> = a.lines().filter(|l| l.contains("noise")).collect();
    assert_eq!(
        noise,
        [
            ".noise_acme out=V(out) src=V1 dec 10 1 1meg",
            ".noise_acme out=V(in) src=V1 dec 5 1 1k"
        ],
        "{a}"
    );
    // Other tests may call it too: at least one call and two cards more.
    assert!(NOISE_CALLS.load(Ordering::SeqCst) > calls);
    assert!(NOISE_CARDS.load(Ordering::SeqCst) >= cards + 2);
    let bad = "* t\nV1 in 0 1 AC 1\nR1 in 0 1k\n.noise v(in)\n.end\n";
    let e = refusal(bad, Lang::NGSPICE, acme);
    assert!(
        e.messages
            .contains("t:4:1: W0201 acme:ACME002 acme's .noise takes one source"),
        "{}",
        e.messages
    );
}

/// Oracle: §6.3's request form: an external hook's cards come back as
/// requests (their spans the written cards), and `splice` puts each
/// answer in (a text) or refuses with it (a code), at the card's line.
#[test]
fn external_hook_requests_and_splice() {
    let ext = register(
        DialectDef::new("ext-tran")
            .extends(Lang::NGSPICE)
            .code("EXT1", Kind::Refusal, "no transient here")
            .external_hook(".tran", 7),
    )
    .expect("registers");
    let deck = "* t\nV1 in 0 1\nR1 in 0 1k\n.tran 1n 1u\n.end\n";
    let lex = Lang::NGSPICE.lexicon().expect("a reader");
    let t = tokenize(deck, lex);
    let ast = parse(&t, lex);
    let src = Source {
        ast: &ast,
        tokens: &t,
        lang: Lang::NGSPICE,
        name: "t",
        kit: None,
        loads: &[],
        numbers: Numbers::Read,
    };
    let p = render_pending(&src, ext).expect("pending");
    assert_eq!(p.requests.len(), 1);
    let r = &p.requests[0];
    assert_eq!(r.hook, 7);
    let span = r.span.start as usize..r.span.end as usize;
    assert_eq!(&p.text[span], ".tran 1n 1u");
    let w = splice(&src, p, &[Answer::Text("tran(1n, 1u)".into())]).expect("spliced");
    assert!(w.text.lines().any(|l| l == "tran(1n, 1u)"), "{}", w.text);
    let p = render_pending(&src, ext).expect("pending");
    let e = splice(&src, p, &[Answer::Refuse("EXT1")]).expect_err("refused");
    assert_eq!(e.messages, "t:4:1: W0201 ext-tran:EXT1 no transient here\n");
}

/// Oracle: metamorphic: an extension of HSPICE that changes only a
/// reading row writes HSPICE's bytes; its own `.step` refusal names its
/// code where HSPICE names the catalogue's.
#[test]
fn my_hspice_extends_hspice() {
    let my = register(my_hspice_as("my-hspice")).expect("registers");
    for d in FIXTURES {
        assert_eq!(
            write(d, Lang::NGSPICE, my),
            write(d, Lang::NGSPICE, Lang::HSPICE),
            "deck:\n{d}"
        );
    }
    let step = "* t\n.param r=1k\nV1 in 0 1\nR1 in 0 {r}\n.step r 1k 2k 500\n.op\n.end\n";
    let e = refusal(step, Lang::NGSPICE_GIVEN, my);
    assert!(
        e.messages
            .contains("W0201 my-hspice:MY001 our HSPICE build has no .step"),
        "{}",
        e.messages
    );
    let e = refusal(step, Lang::NGSPICE_GIVEN, Lang::HSPICE);
    assert!(
        e.diags.iter().any(|d| d.code == Code::W0016),
        "{}",
        e.messages
    );
}

/// Oracle: §5: a plugin extending a plugin copies its rows, codes and
/// hook; its own row overrides the inherited one.
#[test]
fn a_plugin_extends_a_plugin() {
    let acme = acme_lang();
    let two = DialectDef::new("acme-two")
        .extends(acme)
        .write_head(Tag::KwEnds, ".endsubckt");
    let two = register(two).expect("registers");
    assert_eq!(two.base(), Some(acme));
    let a = write(DECK, Lang::NGSPICE, two).expect("written");
    assert!(
        a.contains(".subcircuit load") && a.contains(".endsubckt load"),
        "{a}"
    );
    let deck = "* t\nV1 in 0 1 AC 1\nR1 in 0 1k\n.noise v(in) V1 dec 10 1 1k\n.end\n";
    let n = write(deck, Lang::NGSPICE, two).expect("written");
    assert!(n.contains(".noise_acme"), "{n}");
    let lap = "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s))\nR1 out 0 1k\n.op\n.end\n";
    let e = refusal(lap, Lang::NGSPICE, two);
    assert!(
        e.messages
            .contains("W0201 acme-two:ACME001 acme has no Laplace sources"),
        "{}",
        e.messages
    );
}

/// Oracle: §6.2: a named plugin's masters are respelled after
/// `name (nodes)`, its other text as its base writes it.
#[test]
fn write_master_respells_a_named_master() {
    let sp = DialectDef::new("spectre-res")
        .extends(Lang::SPECTRE)
        .write_master("resistor", "res");
    let sp = register(sp).expect("registers");
    let deck = "* t\nV1 in 0 1\nR1 in 0 1k\n.op\n.end\n";
    let base = write(deck, Lang::NGSPICE, Lang::SPECTRE).expect("written");
    let mine = write(deck, Lang::NGSPICE, sp).expect("written");
    assert!(base.contains(") resistor r="), "{base}");
    assert_eq!(mine, base.replace(") resistor r=", ") res r="));
}

/// Oracle: §5 and §6.5, each registration rule by the call it names.
/// (`extends` of an unregistered language cannot be written: a `Lang`
/// exists only once registered.)
#[test]
fn registration_rules() {
    let held = acme_lang();
    assert_eq!(
        register(acme()),
        Ok(held),
        "an equal definition: the same language"
    );
    assert_eq!(Lang::from_name("ACME"), Some(held));
    let e = |n: &str| DialectDef::new(n).extends(Lang::NGSPICE);
    let cases = [
        (
            acme().scale("y", Scale::Exp(3)),
            RegisterError::Redefined("acme".into()),
        ),
        (e(""), RegisterError::Unnamed),
        (
            DialectDef::new("empty"),
            RegisterError::Unanswered("empty".into(), Construct::ForeignText),
        ),
        (
            e("kind").refuse(Construct::Step, Code::W0030),
            RegisterError::WrongKind("kind".into(), Construct::Step, Code::W0030, Kind::Refusal),
        ),
        (
            e("dotkind").dot_rule("x", DotRule::Note(Code::W0001)),
            RegisterError::DotRuleKind("dotkind".into(), "x".into(), Code::W0001),
        ),
        (
            e("undeclared").refuse_code(Construct::Step, "X1"),
            RegisterError::UnknownCode(
                "undeclared".into(),
                Construct::Step,
                "X1".into(),
                Kind::Refusal,
            ),
        ),
        (
            e("note")
                .code("N1", Kind::Note, "a note")
                .refuse_code(Construct::Step, "N1"),
            RegisterError::UnknownCode("note".into(), Construct::Step, "N1".into(), Kind::Refusal),
        ),
        (
            e("bare").refuse(Construct::Step, Code::W0201),
            RegisterError::UnknownCode(
                "bare".into(),
                Construct::Step,
                "W0201".into(),
                Kind::Refusal,
            ),
        ),
        (
            e("cdl-region").region("cdl", Lang::CDL),
            RegisterError::Readerless("cdl-region".into(), "cdl".into()),
        ),
        (
            acme_as("acme-sp").runs_on(Dialect::Spectre),
            RegisterError::RunsOn("acme-sp".into(), "spectre".into(), "spectre"),
        ),
    ];
    for (def, want) in cases {
        let msg = want.to_string();
        assert_eq!(register(def), Err(want), "{msg}");
    }
}

/// Oracle: §6.5: a plugin is netlist-only unless it says which simulator
/// reads it, allowed when that simulator's language is in its chain.
#[test]
fn runs_on() {
    assert_eq!(acme_lang().runs_on(), None);
    assert_eq!(Lang::HSPICE.runs_on(), Some("hspice"));
    let my = DialectDef::new("my-ngspice")
        .extends(Lang::NGSPICE)
        .runs_on(Dialect::Ngspice);
    let my = register(my).expect("registers");
    assert_eq!(my.runs_on(), Some("ngspice"));
    let deeper = register(DialectDef::new("my-ngspice-2").extends(my)).expect("registers");
    assert_eq!(deeper.runs_on(), Some("ngspice"), "copied by extends");
    let built_in = RunsOn::from(Dialect::Ngspice);
    assert_eq!(built_in.sim, "ngspice");
    assert_eq!(built_in.dialect, Lang::NGSPICE);
}

/// Oracle: note 12 §5's rule, worked by hand on two simulators: one that
/// reads ngspice, one that reads a plugin. A dialect whose `extends`
/// chain holds the simulator's language registers and names it; any
/// other is refused, the simulator and its language named.
#[cfg(feature = "backend")]
#[test]
fn p8_207_runs_on_takes_a_registered_simulator() {
    use spicerack::backend::{
        Installed, Kind as Analysis, Output, Runner, SimDef, by_name, parse_arg, register_simulator,
    };

    let sim = |name: &str, dialect: Lang| {
        register_simulator(SimDef {
            name: name.into(),
            dialect,
            runner: Runner::Command {
                argv: vec![
                    parse_arg(name).expect("a word"),
                    parse_arg("{deck}").expect("a word"),
                ],
                output: Output::NgspiceRaw,
            },
            features: 0,
            analyses: 1 << Analysis::Op as u32,
            installed: Installed::OnPath(name.into()),
        })
        .expect("registers")
    };

    // A simulator that reads ngspice.
    let reads_ng = sim("p8-207-sim", Lang::NGSPICE);
    let on = RunsOn::from(reads_ng);
    assert_eq!(on.sim, "p8-207-sim");
    assert_eq!(on.dialect, Lang::NGSPICE);
    let mine = DialectDef::new("p8-207-ng")
        .extends(Lang::NGSPICE)
        .runs_on(reads_ng);
    let mine = register(mine).expect("ngspice is in its chain");
    assert_eq!(mine.runs_on(), Some("p8-207-sim"));
    assert_eq!(mine.runs_on().and_then(by_name), Some(reads_ng));
    assert!(mine.dump().contains("\nruns_on Some(p8-207-sim)\n"));
    let other = DialectDef::new("p8-207-sp")
        .extends(Lang::SPECTRE)
        .runs_on(reads_ng);
    let want = RegisterError::RunsOn("p8-207-sp".into(), "p8-207-sim".into(), "ngspice");
    assert_eq!(
        want.to_string(),
        "'p8-207-sp': runs_on('p8-207-sim'), but it does not extend 'ngspice', \
         the language simulator 'p8-207-sim' reads"
    );
    assert_eq!(register(other), Err(want));

    // A simulator that reads the plugin: the rule is the simulator's
    // language, not its built-in family.
    let reads_mine = sim("p8-207-deep", mine);
    let child = DialectDef::new("p8-207-child")
        .extends(mine)
        .runs_on(reads_mine);
    let child = register(child).expect("the plugin is in its chain");
    assert_eq!(child.runs_on(), Some("p8-207-deep"));
    let aside = DialectDef::new("p8-207-aside")
        .extends(Lang::NGSPICE)
        .runs_on(reads_mine);
    let want = RegisterError::RunsOn("p8-207-aside".into(), "p8-207-deep".into(), "p8-207-ng");
    assert_eq!(register(aside), Err(want));
}

/// Oracle: metamorphic (determinism): two registration orders give the
/// same bytes, written on 1 and on 8 threads.
#[test]
fn registration_order_and_threads_change_no_byte() {
    let a = |n: &str| acme_as(n).write_head(Tag::KwEnds, ".finis");
    let a1 = register(a("ord-a1")).expect("registers");
    let a2 = register(my_hspice_as("ord-a2")).expect("registers");
    let b2 = register(my_hspice_as("ord-b2")).expect("registers");
    let b1 = register(a("ord-b1")).expect("registers");
    for d in FIXTURES {
        assert_eq!(write(d, Lang::NGSPICE, a1), write(d, Lang::NGSPICE, b1));
        assert_eq!(write(d, Lang::NGSPICE, a2), write(d, Lang::NGSPICE, b2));
    }
    let n = FIXTURES.len();
    let one: Vec<_> = FIXTURES
        .iter()
        .map(|d| write(d, Lang::NGSPICE, a1))
        .collect();
    let eight: Vec<_> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..8)
            .map(|i| s.spawn(move || write(FIXTURES[i % n], Lang::NGSPICE, b1)))
            .collect();
        hs.into_iter().map(|h| h.join().expect("joined")).collect()
    });
    for (i, e) in eight.iter().enumerate() {
        assert_eq!(e, &one[i % n]);
    }
}

/// Oracle: one name registered from 8 threads at once is one language
/// (the name is checked under the registry's lock; this raced before).
#[test]
fn one_name_from_eight_threads_is_one_language() {
    let got: Vec<Lang> = std::thread::scope(|s| {
        let hs: Vec<_> = (0..8)
            .map(|_| s.spawn(|| register(acme_as("acme-race")).expect("registers")))
            .collect();
        hs.into_iter().map(|h| h.join().expect("joined")).collect()
    });
    assert!(got.iter().all(|&l| l == got[0]), "{got:?}");
}

/// Oracle: §6.5: `simulator lang=acme` in a given deck opens a region read
/// by acme's lexicon, kept as acme's text: written as written to acme,
/// refused `W0001` elsewhere.
#[test]
fn a_plugin_region_in_a_given_deck() {
    let acme = acme_lang();
    let deck = "* t\nV1 in 0 1\nR1 in 0 1k\nsimulator lang=acme\n.acme_only 2x ; acme text\nsimulator lang=spice\n.op\n.end\n";
    let lex = Lang::NGSPICE_GIVEN.lexicon().expect("a reader");
    let t = tokenize(deck, lex);
    assert!(
        (0..t.len() as u32).any(|k| t.lexicon_at(k).lang == acme),
        "a token read by acme's lexicon"
    );
    let a = write(deck, Lang::NGSPICE_GIVEN, acme).expect("written");
    assert!(a.contains(".acme_only 2x"), "{a}");
    let e = refusal(deck, Lang::NGSPICE_GIVEN, Lang::NGSPICE);
    assert!(
        e.diags.iter().any(|d| d.code == Code::W0001),
        "{}",
        e.messages
    );
}

/// Oracle: §6.5: a region's name is a whole word: `simulator
/// lang=ngspice-x` opens the plugin `ngspice-x`'s region, not the built-in
/// `ngspice` region its name begins with.
#[test]
fn a_region_name_is_matched_whole() {
    let x = register(acme_as("ngspice-x")).expect("registers");
    let deck =
        "* t\nV1 in 0 1\nsimulator lang=ngspice-x\nR1 in 0 2x\nsimulator lang=spice\n.op\n.end\n";
    let t = tokenize(deck, Lang::NGSPICE_GIVEN.lexicon().expect("a reader"));
    let langs: Vec<Lang> = (0..t.len() as u32).map(|k| t.lexicon_at(k).lang).collect();
    assert!(langs.contains(&x), "a token read by ngspice-x's lexicon");
}

/// Oracle: the pinned dumps (`tests/fixtures/dialects/*.dump`, the Python
/// binding's parity oracle): acme's and my-hspice's rows as registered.
#[test]
fn dumps_match_their_pins() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dialects");
    let my = register(my_hspice_as("my-hspice")).expect("registers");
    for (name, lang) in [("acme", acme_lang()), ("my-hspice", my)] {
        let path = dir.join(format!("{name}.dump"));
        if std::env::var_os("SPICERACK_BLESS").is_some() {
            std::fs::create_dir_all(&dir).expect("a fixture dir");
            std::fs::write(&path, lang.dump()).expect("written");
        }
        let pin = std::fs::read_to_string(&path).expect("a pinned dump");
        assert_eq!(lang.dump(), pin, "{name}");
    }
}
