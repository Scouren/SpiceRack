//! The spine and the seam (`spicerack::dialect`, `spicerack::passes`, `dialect::facade`):
//! `transpile` over the `ir` row on hand-written IR text, a refusal when no pass removes a
//! feature a stub writer lacks, `Policy::Strict` refusing an `Unread`, the facade's message
//! format and code map, the route override, and `Sources::load`'s walk.
//!
//! Oracles: the IR text grammar's worked example (`crates/ir/src/text.rs`) read and written
//! back is the same text (a fixpoint); hand-computed line and column of a byte offset; the old
//! catalogue's own titles; the route rules of `rewrite/SPINE.md` §5 written out as a table; a
//! hand-built include graph with its expected breadth-first order.
//!
//! Tests that reach the IR crate's `todo!()` bodies carry the marker
//! `#[should_panic(expected = "not yet implemented")]`: they fail (an unexpected pass) the day
//! the bodies land, until the marker goes.
//!
//! Contents: tests only.

use std::collections::HashMap;

use spicerack::dialect::facade::{self, Pipeline, ROUTES, UNMAPPED};
use spicerack::dialect::{
    self, BUILTIN, DialectRow, Policy, Reference, STRICT_UNREAD, Sources, Write, irtext::IrText,
};
use spicerack::ir;
use spicerack::syntax::diag::{Code, info};

/// The worked example of `crates/ir/src/text.rs`: a parameter, a subcircuit with two
/// elements, an instance, a source and an analysis, in canonical form.
const WORKED: &str = "\
file \"rc.sp\" deck
  title \"rc divider\"
  param rval = 1k
  subckt div (in out gnd) r=rval
    element resistor r1 (in out) value=r
    element resistor r2 (out gnd) value=(r / 2)
  element instance x1 (a b 0) model=div
  element vsource v1 (a 0) dc=1.8
  analysis op
";

/// A writer that spells every feature but subcircuits.
struct NoSubckt;

static NO_SUBCKT: DialectRow = DialectRow {
    name: "no-subckt",
    text_family: dialect::ANY_FAMILY,
    read: None,
    write: Some(&NoSubckt),
};

impl Write for NoSubckt {
    fn features(&self) -> &'static ir::FeatureSet {
        static SET: std::sync::OnceLock<ir::FeatureSet> = std::sync::OnceLock::new();
        SET.get_or_init(|| {
            let all = IrText.features();
            all.minus(ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Subckt)))
        })
    }

    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, _: &mut ir::Diags) {
        ir::dump(design, file, out);
    }
}

// ---- private ----

fn ir_row() -> &'static DialectRow {
    dialect::by_name("ir").expect("the ir row is built in")
}

#[test]
// marker: IR bodies (slice S1 implementation); remove when they land
#[should_panic(expected = "not yet implemented")]
fn the_worked_example_is_a_fixpoint_of_the_ir_dialect() {
    let src = Sources::one("rc.ir", WORKED);
    let out = dialect::transpile(&src, ir_row(), ir_row(), Policy::Strict)
        .unwrap_or_else(|d| panic!("refused: {:?}", d.rows()));
    assert_eq!(out.text, WORKED);
    assert!(
        out.notes.is_empty(),
        "no note on canonical text: {:?}",
        out.notes.rows()
    );
}

#[test]
// marker: IR bodies (slice S1 implementation); remove when they land
#[should_panic(expected = "not yet implemented")]
fn a_feature_no_pass_removes_is_refused_with_its_index() {
    let src = Sources::one("rc.ir", WORKED);
    let refused = dialect::transpile(&src, ir_row(), &NO_SUBCKT, Policy::Lenient)
        .expect_err("no pass removes subcircuits");
    let subckt = u32::from(ir::Feature::Item(ir::ItemKind::Subckt).index());
    let rows: Vec<(ir::Code, u32)> = refused.rows().iter().map(|d| (d.code, d.arg)).collect();
    assert_eq!(rows, [(ir::diag::UNSUPPORTED, subckt)]);
}

#[test]
// marker: IR bodies (slice S1 implementation); remove when they land
#[should_panic(expected = "not yet implemented")]
fn strict_refuses_an_unread_statement_and_lenient_keeps_it() {
    let text = "file \"o.sp\" deck\n  opaque ngspice unread \".foo bar\"\n";
    let src = Sources::one("o.ir", text);
    let strict = dialect::transpile(&src, ir_row(), ir_row(), Policy::Strict);
    let codes: Vec<ir::Code> = strict
        .expect_err("strict")
        .rows()
        .iter()
        .map(|d| d.code)
        .collect();
    assert!(codes.contains(&STRICT_UNREAD), "{codes:?}");
    let lenient = dialect::transpile(&src, ir_row(), ir_row(), Policy::Lenient);
    assert_eq!(lenient.expect("lenient keeps it").text, text);
}

#[test]
fn the_facade_writes_todays_message_lines_from_spans() {
    // Byte 9 of the text is `c` of line 2 ("ab cd"), column 4; byte 0 is line 1, column 1.
    let src = Sources::one("deck", "first\nab cd\n");
    let at = |start| ir::Span {
        file: ir::FileId(0),
        start,
        len: 1,
    };
    let rows = [
        ir::Diag {
            code: ir::Code::new(b'X', 3),
            span: at(9),
            arg: 0,
        },
        ir::Diag {
            code: ir::Code::new(b'W', 1),
            span: at(0),
            arg: 0,
        },
        ir::Diag {
            code: ir::Code::new(b'P', 2),
            span: ir::Span::NONE,
            arg: 0,
        },
    ];
    let shown = facade::diags(&rows, &src, "t", "spectre");
    let codes: Vec<Code> = shown.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0001, Code::R0042, Code::P0002]);
    let expected = format!(
        "t:1:1: W0001 {}\nt:2:4: R0042 {}\nt:0:0: P0002 {}\n",
        info(Code::W0001).title,
        info(UNMAPPED).title,
        info(Code::P0002).title
    );
    assert_eq!(shown.messages, expected);
}

#[test]
fn a_new_code_with_no_row_is_its_namesake_or_unmapped() {
    let d = |letter, number| ir::Diag {
        code: ir::Code::new(letter, number),
        span: ir::Span::NONE,
        arg: 0,
    };
    assert_eq!(facade::old_code(&d(b'R', 6), "ngspice"), Code::R0006);
    assert_eq!(facade::old_code(&d(b'U', 1), "ngspice"), UNMAPPED);
    assert_eq!(facade::old_code(&d(b'W', 9999), "ngspice"), UNMAPPED);
}

#[test]
fn the_route_table_and_the_override_pick_the_pipeline() {
    // (from, to, forced, new pipeline?) per rewrite/SPINE.md §5.
    let cases = [
        ("ir", "ir", None, false),
        ("ir", "ir", Some(Pipeline::New), true),
        ("ir", "ir", Some(Pipeline::Old), false),
        ("ngspice", "spectre", None, false),
        ("ngspice", "spectre", Some(Pipeline::New), false),
        ("ngspice", "ir", Some(Pipeline::New), true),
        ("spectre", "ir", Some(Pipeline::New), false),
        ("ir", "nowhere", Some(Pipeline::New), false),
    ];
    for (from, to, forced, new) in cases {
        let got = facade::route_with(from, to, forced);
        assert_eq!(got.is_some(), new, "{from} -> {to} under {forced:?}");
        if let Some((f, t)) = got {
            assert_eq!((f.name, t.name), (from, to));
        }
    }
}

#[test]
fn every_listed_route_has_both_sides_in_builtin() {
    for &(from, to) in ROUTES {
        assert!(
            facade::route_with(from, to, Some(Pipeline::New)).is_some(),
            "{from} -> {to}: listed without a reader and a writer in BUILTIN"
        );
        assert!(facade::route_with(from, to, None).is_some());
    }
    assert_eq!(BUILTIN[0].name, "ir", "the IR's own text is the first row");
}

#[test]
fn the_spine_refuses_its_own_strict_code() {
    assert!(dialect::refuses(STRICT_UNREAD));
}

#[test]
fn sources_load_walks_references_breadth_first_once_each() {
    // deck -> a, b; a -> b, sub/c; sub/c -> ../a (seen), d (missing); b names a Verilog-A file.
    let files: HashMap<&str, &str> = HashMap::from([
        (
            "top/deck.ir",
            "file \"deck\" deck\n  include \"a.ir\"\n  include \"./b.ir\"\n",
        ),
        ("top/a.ir", "  include \"b.ir\"\n  include \"sub/c.ir\"\n"),
        ("top/b.ir", "  load veriloga \"r.va\"\n"),
        (
            "top/sub/c.ir",
            "  include \"../a.ir\"\n  include \"d.ir\"\n",
        ),
    ]);
    let mut asked = Vec::new();
    let mut loader = |p: &str| {
        asked.push(p.to_string());
        files.get(p).map(|t| t.to_string())
    };
    let src = Sources::load(&["top/deck.ir"], ir::Role::Deck, &IrText, &mut loader);
    let paths: Vec<&str> = src.files().iter().map(|(p, _)| p.as_str()).collect();
    assert_eq!(
        paths,
        ["top/deck.ir", "top/a.ir", "top/b.ir", "top/sub/c.ir"]
    );
    assert_eq!(src.entries().len(), 1);
    assert_eq!(src.form(), ir::Role::Deck);
    assert_eq!(asked.last().map(String::as_str), Some("top/sub/d.ir"));
    assert!(
        !asked.iter().any(|p| p.ends_with("r.va")),
        "Verilog-A is not followed"
    );
}

#[test]
fn sources_load_stops_at_the_depth_cap() {
    // f0 -> f1 -> ... : DEPTH_MAX + 1 files are read (depths 0..=DEPTH_MAX).
    let n = usize::from(dialect::DEPTH_MAX) + 20;
    let texts: Vec<String> = (0..n)
        .map(|i| format!("  include \"f{}.ir\"\n", i + 1))
        .collect();
    let mut loader = |p: &str| {
        let i: usize = p.strip_prefix('f')?.strip_suffix(".ir")?.parse().ok()?;
        texts.get(i).cloned()
    };
    let src = Sources::load(&["f0.ir"], ir::Role::Library, &IrText, &mut loader);
    assert_eq!(src.files().len(), usize::from(dialect::DEPTH_MAX) + 1);
    assert_eq!(src.form(), ir::Role::Library);
}

#[test]
fn a_reference_resolves_beside_its_file() {
    let r = Reference::File("../m/x.lib".into());
    assert_eq!(r.resolve("kit/deck/a.sp"), "kit/m/x.lib");
    assert_eq!(
        Reference::File("/abs/y.lib".into()).resolve("kit/a.sp"),
        "/abs/y.lib"
    );
    assert_eq!(Reference::VerilogA("r.va".into()).resolve("a.sp"), "r.va");
}
