//! Laws L1, L2 and L6 over VACASK's own corpus (`$VACASK_CORPUS`, 184 netlists): each file reads
//! without a panic and with no `X` row (an `ir::check` violation is a reader bug), and is a
//! fixpoint of read, write, read. A `.sim` is read as a deck, a `.lib` or `.inc` as a library.
//!
//! Oracle: the laws (REWRITE-PLAN §4.2); the canonical IR text (`ir::dump`) as `≅`.
//!
//! Contents: tests only.

use std::path::Path;

use spicerack::dialect::{self, Policy, Sources};
use spicerack::ir;

/// Refusals the reader gives in corpus files VACASK runs, each `file code`, in file order;
/// strict both ways (a reader bug each, or a file VACASK rejects: say which beside it).
#[rustfmt::skip] // a table: one row per line
const KNOWN_INVALID: &[&str] = &[
    // VACASK 36be560 rejects a `;`-separated list (settle run/list_semicolon: "Parser syntax
    // error, unexpected ;, expecting ] or \",\""): these decks are a newer VACASK's.
    "demo/spice/vdmossh.sim P2320",
    // VACASK 36be560's shipped resistor has no `has_noise` (settle run/resistor_has_noise:
    // "Parameter 'has_noise' not found.").
    "test/test_ac.sim P2103",
    "test/test_acxf.sim P2103",
    "test/test_noise.sim P2103",
    // the `;`-separated list again
    "test/test_opnodeset.sim P2320",
    "test/test_tranic.sim P2320",
];

// ---- private ----

/// `text` as the only file of a read, in `role`.
fn sources(name: &str, text: &str, role: ir::Role) -> Sources {
    let loader = |_: &str| Some(text.to_string());
    let row = dialect::by_name("vacask").expect("the vacask row");
    Sources::load(
        &[name],
        role,
        row.read.expect("a reader"),
        None,
        &mut |p: &str| (p == name).then(|| loader(p)).flatten(),
    )
}

/// The canonical IR text of every file.
fn canonical(design: &ir::Design) -> String {
    let mut out = String::new();
    for f in design.files() {
        ir::dump(design, f, &mut out);
    }
    out
}

#[test]
fn every_corpus_file_reads_cleanly_and_is_a_fixpoint() {
    let root = std::env::var("VACASK_CORPUS")
        .expect("VACASK_CORPUS not set: run the tests in `nix develop .#testing`");
    let root = Path::new(&root);
    let row = dialect::by_name("vacask").expect("the vacask row");
    let mut todo = vec![root.to_path_buf()];
    let mut files = Vec::new();
    while let Some(dir) = todo.pop() {
        for e in std::fs::read_dir(&dir).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() && p.file_name().is_some_and(|n| n != "gnucap") {
                todo.push(p);
            } else if p
                .extension()
                .is_some_and(|x| x == "sim" || x == "lib" || x == "inc")
            {
                files.push(p);
            }
        }
    }
    files.sort();
    let mut failed = Vec::new();
    let mut unread_files: Vec<String> = Vec::new();
    let mut invalid: Vec<String> = Vec::new();
    for path in &files {
        let rel = path.strip_prefix(root).unwrap().display().to_string();
        let text = std::fs::read_to_string(path).unwrap();
        let role = match path.extension().is_some_and(|x| x == "sim") {
            true => ir::Role::Deck,
            false => ir::Role::Library,
        };
        let src = sources(&rel, &text, role);
        let (mut once, mut diags) = (ir::Design::new(), ir::Diags::new());
        dialect::read(&src, row, Policy::Lenient, &mut once, &mut diags);
        if diags.rows().iter().any(|d| d.code.letter == b'X') {
            failed.push(format!("{rel}: X rows {:?}", diags.rows()));
            continue;
        }
        // F11: in a deck, a module model whose module no definition binds is provided only by a
        // plain `load` (one that loads every module of its file), never by a family's library
        // (a library file's loads are its including deck's).
        let reach = once.reachable();
        let loads = reach.iter().any(|(at, _)| at.kind == ir::ItemKind::Load);
        let orphan = reach.iter().any(|(at, _)| {
            at.kind == ir::ItemKind::Model && {
                let m: ir::Model = once.row(at.index);
                m.family == ir::ModelFamily::Module && m.module.def == ir::DefId::NONE
            }
        });
        if orphan && !loads && role == ir::Role::Deck {
            failed.push(format!("{rel}: a module model no load provides"));
        }
        let before = canonical(&once);
        // The zero gate (item 6b): every corpus statement is read, Private, Inert, a hint, or
        // invalid in source; none is left `Unread`.
        let unread = before.matches("opaque vacask unread").count();
        if unread > 0 {
            unread_files.push(format!("{rel}: {unread}"));
        }
        // The census (V1-V3): VACASK runs every corpus file but the ngspice- and Xyce-format
        // benchmark files, so none of its statements is invalid in source.
        let foreign = ["/ngspice", "/xyce"].iter().any(|d| rel.contains(d));
        if !foreign {
            let mut codes: Vec<String> = diags
                .rows()
                .iter()
                .filter(|d| dialect::refuses(d.code))
                .map(|d| format!("{rel} {}", d.code))
                .collect();
            codes.dedup();
            invalid.extend(codes);
        }
        let mut written = String::new();
        let mut wd = ir::Diags::new();
        dialect::legalize(&mut once, row, &mut wd);
        written.push_str(&dialect::write(&once, once.entry(), row, &mut wd));
        let (mut twice, mut d2) = (ir::Design::new(), ir::Diags::new());
        dialect::read(
            &sources(&rel, &written, role),
            row,
            Policy::Lenient,
            &mut twice,
            &mut d2,
        );
        if canonical(&twice) != before {
            failed.push(format!(
                "{rel}: not a fixpoint\n--- once\n{before}--- twice\n{}",
                canonical(&twice)
            ));
        }
    }
    let known: Vec<String> = KNOWN_INVALID.iter().map(|s| s.to_string()).collect();
    assert_eq!(
        invalid, known,
        "refusals in decks VACASK runs (file, code), against KNOWN_INVALID"
    );
    assert!(
        unread_files.is_empty(),
        "{} files hold Unread statements:\n{}",
        unread_files.len(),
        unread_files.join("\n")
    );
    assert!(
        failed.is_empty(),
        "{} of {} files:\n{}",
        failed.len(),
        files.len(),
        failed.join("\n")
    );
}
