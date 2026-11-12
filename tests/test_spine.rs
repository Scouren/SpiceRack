//! The spine and the seam (`spicerack::dialect`, `spicerack::passes`, `dialect::facade`):
//! `transpile` over the `ir` row on hand-written IR text, a refusal when no pass removes a
//! feature a stub writer lacks, `Policy::Strict` refusing an `Unread`, the facade's message
//! format and code map, the route override, and `Sources::load`'s walk.
//!
//! Oracles: canonical IR text read and written back is the same text (a fixpoint): the
//! kernel's kinds (`KERNEL`) and the grammar's worked example (`crates/ir/src/text.rs`); hand-computed line and column of a byte offset; the old
//! catalogue's own titles; the route rules of `rewrite/SPINE.md` §5 written out as a table; a
//! hand-built include graph with its expected breadth-first order.

//!
//! Contents: tests only.

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};

use spicerack::dialect::facade::{self, IR_NOTE, IR_REFUSAL, Pipeline, ROUTES};
use spicerack::dialect::{
    self, BUILTIN, DialectRow, HINT_DROPPED, INERT_DROPPED, Policy, Reference, STRICT_UNREAD,
    Sources, Write, irtext::IrText,
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

/// The kernel's own kinds only (title, comment, global, include, param, func, subckt, if,
/// section, opaque), in canonical form: what the IR bodies landed so far can read and write.
const KERNEL: &str = "\
file \"kit.sp\" deck
  title \"kernel kinds\"
  comment \"a note\"
  global vdd vss
  include \"models.lib\" section=tt
  param w = 2u
  param l = w / 2
  func area(a, b) = a * b
  subckt cell (a b) r=w
    param k = r * 2
  if (w > 1u)
    param m = 1
  elif (w > 0)
    param m = 2
  else
    param m = 3
  opaque ngspice inert \".width out=80\"
file \"models.lib\" library
  section tt
    param vth = 0.4
";

/// A writer that spells every feature but subcircuits.
struct NoSubckt;

static NO_SUBCKT: DialectRow = DialectRow {
    name: "no-subckt",
    text_family: dialect::ANY_FAMILY,
    read: None,
    write: Some(&NoSubckt),
};

/// A writer that refuses every comment reading "a note" in `admits` (a reason no feature
/// states), and counts its write calls.
struct Picky(AtomicUsize);

/// The picky writer's refusal: a code of its own (no catalogue holds it, so it refuses).
const PICKY_CODE: ir::Code = ir::Code::new(b'W', 9001);

/// A writer of one text family that counts its calls and spells everything. Each test makes its
/// own ([`counting`]), so tests running in parallel never share a counter.
struct Counting(AtomicUsize);

/// Two files: an inert ngspice statement in the deck, a private one in the library.
const TWO_FILES: &str = "\
file \"a.sp\" deck
  opaque ngspice inert \".width out=80\"
  title \"a\"
file \"b.lib\" library
  opaque ngspice private \".csparam x=1\"
";

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

impl Write for Counting {
    fn features(&self) -> &'static ir::FeatureSet {
        IrText.features()
    }

    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, _: &mut ir::Diags) {
        self.0.fetch_add(1, Ordering::Relaxed);
        ir::dump(design, file, out);
    }
}

impl Write for Picky {
    fn features(&self) -> &'static ir::FeatureSet {
        IrText.features()
    }

    fn write(&self, design: &ir::Design, file: ir::FileId, out: &mut String, _: &mut ir::Diags) {
        self.0.fetch_add(1, Ordering::Relaxed);
        ir::dump(design, file, out);
    }

    fn admits(&self, design: &ir::Design, diags: &mut ir::Diags) {
        let table = <ir::item::Comment as ir::ItemRow>::table(design);
        for (i, row) in table.rows().iter().enumerate() {
            if design.text(row.text) == "a note" {
                diags.push(PICKY_CODE, table.span(i as u32), 0);
            }
        }
    }
}

// ---- private ----

/// A fresh counting writer of `family` and its row (leaked: a row holds `&'static dyn Write`).
fn counting(family: &'static str) -> (&'static DialectRow, &'static Counting) {
    let stub: &'static Counting = Box::leak(Box::new(Counting(AtomicUsize::new(0))));
    let row = Box::leak(Box::new(DialectRow {
        name: "counting-stub",
        text_family: family,
        read: None,
        write: Some(stub),
    }));
    (row, stub)
}

/// A fresh picky writer and its row, as [`counting`].
fn picky() -> (&'static DialectRow, &'static Picky) {
    let stub: &'static Picky = Box::leak(Box::new(Picky(AtomicUsize::new(0))));
    let row = Box::leak(Box::new(DialectRow {
        name: "picky",
        text_family: dialect::ANY_FAMILY,
        read: None,
        write: Some(stub),
    }));
    (row, stub)
}

fn ir_row() -> &'static DialectRow {
    dialect::by_name("ir").expect("the ir row is built in")
}

#[test]
fn kernel_kinds_are_a_fixpoint_of_the_ir_dialect() {
    let src = Sources::one("kit.ir", KERNEL);
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, ir_row(), Policy::Strict, &mut design, &mut diags);
    assert!(
        diags.is_empty(),
        "no row on canonical text: {:?}",
        diags.rows()
    );
    let written: String = design
        .files()
        .map(|f| dialect::write(&design, f, ir_row(), &mut diags))
        .collect();
    assert_eq!(written, KERNEL);
    let out = dialect::transpile(&src, ir_row(), ir_row(), Policy::Strict)
        .unwrap_or_else(|d| panic!("refused: {:?}", d.rows()));
    assert_eq!(
        out.text,
        KERNEL
            .split("file \"models.lib\"")
            .next()
            .expect("the deck")
    );
}

#[test]
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
fn a_feature_no_pass_removes_is_refused_at_its_item() {
    let src = Sources::one("kit.ir", KERNEL);
    let refused = dialect::transpile(&src, ir_row(), &NO_SUBCKT, Policy::Lenient)
        .expect_err("no pass removes subcircuits");
    let subckt = u32::from(ir::Feature::Item(ir::ItemKind::Subckt).index());
    let rows: Vec<(ir::Code, u32)> = refused.rows().iter().map(|d| (d.code, d.arg)).collect();
    assert_eq!(rows, [(ir::diag::UNSUPPORTED, subckt)]);
    let at = refused.rows()[0].span;
    assert_eq!(
        &KERNEL[at.start as usize..][..6],
        "subckt",
        "the row is at the subckt's line"
    );
}

#[test]
fn a_feature_no_pass_removes_is_refused_with_its_index() {
    let src = Sources::one("rc.ir", WORKED);
    let refused = dialect::transpile(&src, ir_row(), &NO_SUBCKT, Policy::Lenient)
        .expect_err("no pass removes subcircuits");
    let subckt = u32::from(ir::Feature::Item(ir::ItemKind::Subckt).index());
    let rows: Vec<(ir::Code, u32)> = refused.rows().iter().map(|d| (d.code, d.arg)).collect();
    assert_eq!(rows, [(ir::diag::UNSUPPORTED, subckt)]);
}

#[test]
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
fn legalize_drops_a_foreign_inert_and_refuses_a_foreign_private_once() {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one("two.ir", TWO_FILES),
        ir_row(),
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    let read_rows = diags.len();
    dialect::legalize(&mut design, counting("spectre").0, &mut diags);
    let opaque = u32::from(ir::Feature::Item(ir::ItemKind::Opaque).index());
    let rows: Vec<(ir::Code, u32)> = diags.rows()[read_rows..]
        .iter()
        .map(|d| (d.code, d.arg))
        .collect();
    assert_eq!(rows, [(INERT_DROPPED, 0), (ir::diag::UNSUPPORTED, opaque)]);
    let deck = design.find_file("a.sp");
    let left = design.items(design.file(deck).top);
    assert_eq!(
        left.iter().map(|i| i.kind).collect::<Vec<_>>(),
        [ir::ItemKind::Title]
    );
}

#[test]
fn after_one_clean_legalize_each_file_is_written_once() {
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(
        &Sources::one("two.ir", TWO_FILES),
        ir_row(),
        Policy::Lenient,
        &mut design,
        &mut diags,
    );
    let read_rows = diags.len();
    let (ngspice, stub) = counting("ngspice");
    dialect::legalize(&mut design, ngspice, &mut diags);
    let added = &diags.rows()[read_rows..];
    assert!(added.is_empty(), "the ngspice family keeps both: {added:?}");
    let texts: Vec<String> = design
        .files()
        .map(|f| dialect::write(&design, f, ngspice, &mut diags))
        .collect();
    assert_eq!(texts.concat(), TWO_FILES);
    assert_eq!(stub.0.load(Ordering::Relaxed), 2, "once per file");
}

#[test]
fn a_writer_admits_refusal_comes_last_from_legalize_and_stops_transpile() {
    let src = Sources::one("kit.ir", KERNEL);
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    dialect::read(&src, ir_row(), Policy::Lenient, &mut design, &mut diags);
    let read_rows = diags.len();
    let (row, stub) = picky();
    dialect::legalize(&mut design, row, &mut diags);
    let added: Vec<ir::Code> = diags.rows()[read_rows..].iter().map(|d| d.code).collect();
    assert_eq!(added, [PICKY_CODE]);
    let at = diags.rows()[read_rows].span;
    assert_eq!(
        &KERNEL[at.start as usize..][..7],
        "comment",
        "the row is at the comment"
    );
    let refused = dialect::transpile(&src, ir_row(), row, Policy::Lenient);
    let codes: Vec<ir::Code> = refused
        .expect_err("admits refused")
        .rows()
        .iter()
        .map(|d| d.code)
        .collect();
    assert_eq!(codes, [PICKY_CODE]);
    assert_eq!(
        stub.0.load(Ordering::Relaxed),
        0,
        "a refused design is never written"
    );
}

#[test]
fn a_writer_that_admits_everything_writes_as_before() {
    let src = Sources::one("kit.ir", KERNEL);
    let out = dialect::transpile(&src, ir_row(), counting("ngspice").0, Policy::Lenient)
        .unwrap_or_else(|d| panic!("refused: {:?}", d.rows()));
    assert_eq!(
        out.text,
        KERNEL
            .split("file \"models.lib\"")
            .next()
            .expect("the deck")
    );
}

#[test]
fn a_routed_ir_text_diagnostic_names_its_line() {
    let text = "file \"d.sp\" deck\n  title \"t\"\n  bogus line\n";
    let refused = facade::render_text(text, "d", ir_row(), ir_row()).expect_err("P9001 refuses");
    let line = refused.messages.lines().next().unwrap_or("");
    assert!(
        line.starts_with("d:3:"),
        "a routed diagnostic names its line: {line}"
    );
}

#[test]
fn legalize_drops_a_foreign_hint_with_its_own_note_and_the_family_keeps_it() {
    let text =
        "file \"h.sp\" deck\n  title \"h\"\n  opaque ngspice hint \".options reltol=1e-4\"\n";
    let read = |design: &mut ir::Design, diags: &mut ir::Diags| {
        dialect::read(
            &Sources::one("h.ir", text),
            ir_row(),
            Policy::Lenient,
            design,
            diags,
        );
    };
    let (mut design, mut diags) = (ir::Design::new(), ir::Diags::new());
    read(&mut design, &mut diags);
    let at = diags.len();
    dialect::legalize(&mut design, counting("spectre").0, &mut diags);
    let rows: Vec<(ir::Code, u32)> = diags.rows()[at..].iter().map(|d| (d.code, d.arg)).collect();
    assert_eq!(rows, [(HINT_DROPPED, 0)]);
    let kinds: Vec<ir::ItemKind> = design.reachable().iter().map(|(i, _)| i.kind).collect();
    assert_eq!(kinds, [ir::ItemKind::Title], "the hint is gone");
    let (mut design, mut diags) = (ir::Design::new(), ir::Diags::new());
    read(&mut design, &mut diags);
    let at = diags.len();
    dialect::legalize(&mut design, counting("ngspice").0, &mut diags);
    assert!(diags.rows()[at..].is_empty(), "its own family keeps it");
    assert_eq!(design.reachable().len(), 2);
    assert!(dialect::info(HINT_DROPPED).is_some_and(|i| i.kind == ir::Kind::Note));
}

#[test]
fn a_kit_whose_entry_is_a_library_writes_the_library() {
    let text = "file \"m.lib\" library\n  param vth = 0.4\nfile \"d.sp\" deck\n  title \"d\"\n";
    let out = dialect::transpile(
        &Sources::one("m.ir", text),
        ir_row(),
        ir_row(),
        Policy::Lenient,
    )
    .unwrap_or_else(|d| panic!("refused: {:?}", d.rows()));
    assert!(
        out.text.starts_with("file \"m.lib\""),
        "transpile writes the library: {}",
        out.text
    );
}

#[test]
fn the_facade_writes_todays_message_lines_from_spans() {
    // Byte 9 of the text is `c` of line 2 ("ab cd"), column 4; byte 0 is line 1, column 1.
    let src = Sources::one("deck", "first\nab cd\n");
    let at = |file, start| ir::Span {
        file: ir::FileId(file),
        start,
        len: 1,
    };
    let strict = |span| ir::Diag {
        code: STRICT_UNREAD,
        span,
        arg: 0,
    };
    let rows = [
        strict(at(0, 9)),
        strict(ir::Span::NONE),
        strict(at(0, 0)),
        strict(at(7, 0)),
    ];
    let shown = facade::diags(&rows, &src, "t", "spectre");
    let codes: Vec<Code> = shown.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0203; 4], "a refusal stays a refusal");
    let title = dialect::info(STRICT_UNREAD).expect("the spine's row").title;
    let expected = format!(
        "t:1:1: W0203 S0101 {title}\nt:2:4: W0203 S0101 {title}\nt:0:0: W0203 S0101 {title}\nt:0:0: W0203 S0101 {title}\n"
    );
    assert_eq!(shown.messages, expected);
    assert_eq!(
        info(Code::W0203).kind,
        spicerack::syntax::diag::Kind::Refusal
    );
    assert_eq!(info(Code::W0204).kind, spicerack::syntax::diag::Kind::Note);
}

#[test]
fn a_new_code_with_no_row_is_shown_by_its_verdict() {
    let d = |code| ir::Diag {
        code,
        span: ir::Span::NONE,
        arg: 0,
    };
    let unread = ir::diag::opaque_code(ir::OpaqueClass::Unread);
    assert_eq!(facade::old_code(&d(unread), "ngspice"), IR_NOTE);
    assert_eq!(
        facade::old_code(&d(ir::diag::UNSUPPORTED), "ngspice"),
        IR_REFUSAL
    );
    // A code no catalogue holds refuses, loudly.
    assert_eq!(
        facade::old_code(&d(ir::Code::new(b'W', 9999)), "ngspice"),
        IR_REFUSAL
    );
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
