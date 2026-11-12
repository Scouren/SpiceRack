//! The front end the VACASK reader uses as its private syntax tree (DIALECT-VACASK §3.1): every
//! byte of every corpus file is in a token, or in a gap that holds only what VACASK itself
//! ignores (blanks, `//` and `/* */` comments, a `\` before a newline). Law L1 counts statements
//! through that tree, so a byte in neither is text the reader would drop unseen.
//!
//! Oracle: VACASK's lexical rules as its docs and `lib/dfllexer.l` state them (`syntax/dialects/
//! vacask.rs` cites the lines), checked byte by byte, independently of the tokenizer; the files
//! where it does not hold are named in `tests/fixtures/dialect/vacask/probe-known.txt`, compared
//! as a list (an unexpected pass fails until its line goes).
//!
//! Contents: tests only.

use std::path::{Path, PathBuf};

use spicerack::syntax::token::Tag;
use spicerack::syntax::{Lang, tokenize};

/// The corpus extensions that hold VACASK netlists.
const EXTENSIONS: &[&str] = &["sim", "lib", "inc"];
/// Directories of another simulator's decks inside VACASK's tree (VS V35).
const OUT_OF_SET: &str = "gnucap";
/// Files the corpus walk may find, at most.
const FILES_MAX: usize = 4096;

// ---- private ----

/// Every VACASK netlist under `root`, sorted.
fn corpus(root: &Path) -> Vec<PathBuf> {
    let mut todo = vec![root.to_path_buf()];
    let mut out = Vec::new();
    while let Some(dir) = todo.pop() {
        for entry in std::fs::read_dir(&dir).expect("a readable corpus directory") {
            let path = entry.expect("a directory entry").path();
            if path.is_dir() {
                if path.file_name().is_none_or(|n| n != OUT_OF_SET) {
                    todo.push(path);
                }
            } else if path
                .extension()
                .is_some_and(|e| EXTENSIONS.iter().any(|x| e == *x))
            {
                out.push(path);
            }
            assert!(
                out.len() <= FILES_MAX,
                "corpus walk: more than {FILES_MAX} files"
            );
        }
    }
    out.sort();
    out
}

/// The first byte of `text` outside every token and every ignorable gap, as `line: text`; `None`
/// when there is none.
fn first_unread(text: &str) -> Option<String> {
    let lex = Lang::VACASK.lexicon().expect("VACASK has a lexicon");
    let tokens = tokenize(text, lex);
    let bytes = text.as_bytes();
    let mut at = 0usize;
    for (i, tag) in tokens.tags().iter().enumerate() {
        let i = u32::try_from(i).expect("fewer than 2^32 tokens");
        let (start, end) = (tokens.start(i) as usize, tokens.end(i) as usize);
        if *tag == Tag::Invalid {
            return Some(report(text, start));
        }
        if start > at
            && let Some(bad) = unread_in_gap(bytes, at, start)
        {
            return Some(report(text, bad));
        }
        at = at.max(end);
    }
    unread_in_gap(bytes, at, bytes.len()).map(|bad| report(text, bad))
}

/// The first byte of `bytes[from..to]` that is neither a blank, nor in a comment, nor a `\`
/// that ends its line.
fn unread_in_gap(bytes: &[u8], from: usize, to: usize) -> Option<usize> {
    let mut i = from;
    while i < to {
        let rest = &bytes[i..to];
        if rest[0].is_ascii_whitespace() {
            i += 1;
        } else if rest.starts_with(b"//") {
            i += rest.iter().position(|&b| b == b'\n').unwrap_or(rest.len());
        } else if rest.starts_with(b"/*") {
            let Some(close) = rest.windows(2).position(|w| w == b"*/") else {
                return Some(i);
            };
            i += close + 2;
        } else if rest[0] == b'\\' && (rest.get(1) == Some(&b'\n') || rest.starts_with(b"\\\r\n")) {
            i += 1;
        } else {
            return Some(i);
        }
    }
    None
}

/// `line: the rest of that line` for byte `at`.
fn report(text: &str, at: usize) -> String {
    let line = text[..at].bytes().filter(|&b| b == b'\n').count() + 1;
    let end = text[at..].find('\n').map_or(text.len(), |n| at + n);
    format!("{line}: {}", text[at..end].trim_end())
}

#[test]
fn every_corpus_byte_is_in_a_token_or_an_ignorable_gap() {
    let root = std::env::var("VACASK_CORPUS")
        .expect("VACASK_CORPUS not set: run the tests in `nix develop .#testing`");
    let root = Path::new(&root);
    let files = corpus(root);
    let lib = root.join("demo/cmos180n.lib");
    assert!(files.contains(&lib), "the walk misses {}", lib.display());
    let mut found = Vec::new();
    for path in &files {
        let text = std::fs::read_to_string(path).expect("a UTF-8 corpus file");
        if let Some(bad) = first_unread(&text) {
            let rel = path.strip_prefix(root).expect("under the root");
            found.push(format!("{} {bad}", rel.display()));
        }
    }
    let known = include_str!("../fixtures/dialect/vacask/probe-known.txt");
    let known: Vec<&str> = known
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    assert_eq!(
        found,
        known,
        "files with bytes outside every token and gap ({} read)",
        files.len()
    );
}

#[test]
fn the_gap_check_sees_a_dropped_byte() {
    // Oracle: hand-written gaps (the check's own red case).
    let b = b"a // c\n/* d */ \\\n e";
    assert_eq!(unread_in_gap(b, 1, b.len() - 1), None);
    assert_eq!(unread_in_gap(b"  x", 0, 3), Some(2));
    assert_eq!(unread_in_gap(b"/* open", 0, 7), Some(0));
}
