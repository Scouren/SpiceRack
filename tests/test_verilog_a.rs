//! A Spectre kit's Verilog-A modules as `syntax::resolve` definitions: the
//! made-up kit `tests/fixtures/verilog_a/` (a varactor module, a subckt
//! wrapping it) read from disk, every instance bound.
//!
//! Oracle: the fixture's text, read by hand. Spectre UG p.118
//! (`ahdl_include`), p.66 (an instance's master); IEEE 1364-2005 §12.1
//! (the port list), §4.10 (`parameter`, `localparam`).
//!
//! Contents: none (tests only).

use std::path::Path;

use spicerack::import;
use spicerack::syntax::ast::{Node, Tag};
use spicerack::syntax::diag::Code;
use spicerack::syntax::render::render_kit;
use spicerack::syntax::resolve::{Binding, DefKind, FileId, Form, Role};
use spicerack::syntax::{Dialect, lexicon, parse, tokenize};

// ---- private ----

#[test]
fn a_spectre_kit_binds_its_verilog_a_instances() {
    // kit.inc loads va/varactor.va at its top; `xv` (in subckt varcap) and
    // `xtop` name its module `varactor_va`, whose ports are `p n` and
    // whose parameters are `area c0 vbi` (`two` is a localparam); `xcap`
    // names the subckt. No `R` row: every master resolves and each node
    // count matches. Checks: 6.
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/verilog_a");
    let kit = import::kit(
        &[dir.join("kit.inc")],
        Form::Library,
        Dialect::Spectre,
        &dir,
    );
    let roles: Vec<Role> = kit.files().iter().map(|f| f.role).collect();
    assert_eq!(roles, [Role::Library, Role::VerilogA]);
    let file = kit.file(FileId(0));
    let t = tokenize(&file.text, &lexicon::SPECTRE);
    let ast = parse(&t, &lexicon::SPECTRE);
    let mut masters = Vec::new();
    for n in (0..ast.len() as u32).map(Node) {
        if ast.tag(n) != Tag::Instance {
            continue;
        }
        let name = t.text(ast.main_token(Node(ast.header(n).name))).to_string();
        match kit.binding(FileId(0), n) {
            Some(Binding::Def(d)) => masters.push((name, kit.definition(d))),
            b => panic!("{name}: {b:?}"),
        }
    }
    let names: Vec<(&str, &str)> = masters
        .iter()
        .map(|(n, d)| (n.as_str(), d.name.as_str()))
        .collect();
    assert_eq!(
        names,
        [
            ("xv", "varactor_va"),
            ("xtop", "varactor_va"),
            ("xcap", "varcap")
        ]
    );
    let module = masters[0].1;
    assert_eq!(
        module.kind,
        DefKind::VerilogA {
            ports: vec!["p".into(), "n".into()],
            params: vec!["area".into(), "c0".into(), "vbi".into()],
        }
    );
    // Declared at the `ahdl_include` of kit.inc, not in the source.
    assert_eq!(module.file, FileId(0));
    assert_eq!(file.diags, []);
    // The writer gets no `R` row (it still copies the instances of a
    // module as written, W0027: render has no card for one yet).
    let w = render_kit(&kit, FileId(0), "kit", Dialect::Spectre).expect("written");
    let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
    assert_eq!(codes, [Code::W0027, Code::W0027], "{}", w.notes.messages);
}
