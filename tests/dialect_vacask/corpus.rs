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

// ---- private ----

/// `text` as the only file of a read, in `role`.
fn sources(name: &str, text: &str, role: ir::Role) -> Sources {
    let loader = |_: &str| Some(text.to_string());
    let row = dialect::by_name("vacask").expect("the vacask row");
    Sources::load(
        &[name],
        role,
        row.read.expect("a reader"),
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
        let before = canonical(&once);
        // The zero gate (item 6b): every corpus statement is read, Private, Inert, a hint, or
        // invalid in source; none is left `Unread`.
        let unread = before.matches("opaque vacask unread").count();
        if unread > 0 {
            unread_files.push(format!("{rel}: {unread}"));
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
