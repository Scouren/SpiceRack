//! The control tests' ngspice runs: a deck → its raw file's bytes on ngspice-45, without the
//! `Date:` lines (for `tests/dialect_ngspice_control.rs`, split from it at its size limit).
//!
//! Cites: ngspice-45 `ivars.c` (`SPICE_ASCIIRAWFILE`), `commands.c` (`write`).
//!
//! Contents: `pub(crate) fn raw(&Path, &str, &str) -> Vec<u8>`,
//! `pub(crate) fn raw_r(&Path, &str, &str) -> Vec<u8>`

use std::path::Path;

/// Every plot `deck` makes on ngspice-45 (`ngspice -b`), written to one raw file by a block
/// appended before `.end` (the test's instrumentation, never the writer's), without its
/// `Date:` lines. Asserts the run exits 0 and writes one.
pub(crate) fn raw(dir: &Path, stem: &str, deck: &str) -> Vec<u8> {
    let cir = dir.join(format!("{stem}.cir"));
    let out = dir.join(format!("{stem}.raw"));
    let _ = std::fs::remove_file(&out);
    let block = format!(
        ".control\nset appendwrite\nforeach p $plots\nstrcmp _t_const $p const\nif $_t_const ne 0\nsetplot $p\nwrite {}\nend\nend\n.endc\n",
        out.display()
    );
    let at = deck
        .rfind(".end\n")
        .expect("a deck ends with .end (the fixtures do)");
    std::fs::write(&cir, format!("{}{block}{}", &deck[..at], &deck[at..])).expect("the deck");
    let run = std::process::Command::new("ngspice")
        .arg("-b")
        .arg(&cir)
        .output()
        .expect("ngspice runs (nix develop .#testing)");
    assert!(
        run.status.success(),
        "{stem}: ngspice failed\n{}",
        String::from_utf8_lossy(&run.stderr)
    );
    let bytes = std::fs::read(&out).unwrap_or_else(|e| panic!("{stem}: no raw file: {e}"));
    let mut kept = Vec::with_capacity(bytes.len());
    let mut at = 0;
    // Bounded: each step moves `at` forward.
    while at < bytes.len() {
        let line_start = at == 0 || bytes[at - 1] == b'\n';
        if line_start && bytes[at..].starts_with(b"Date:") {
            at += bytes[at..]
                .iter()
                .position(|&b| b == b'\n')
                .map_or(bytes.len() - at, |k| k + 1);
            continue;
        }
        kept.push(bytes[at]);
        at += 1;
    }
    kept
}

/// The raw file `ngspice -b -r` writes for `deck`, ASCII (`SPICE_ASCIIRAWFILE`, ivars.c), without
/// its `Date:` lines.
pub(crate) fn raw_r(dir: &Path, stem: &str, deck: &str) -> Vec<u8> {
    let cir = dir.join(format!("{stem}.cir"));
    let out = dir.join(format!("{stem}.raw"));
    let _ = std::fs::remove_file(&out);
    std::fs::write(&cir, deck).expect("the deck");
    let run = std::process::Command::new("ngspice")
        .env("SPICE_ASCIIRAWFILE", "1")
        .arg("-b")
        .arg("-r")
        .arg(&out)
        .arg(&cir)
        .output()
        .expect("ngspice runs (nix develop .#testing)");
    assert!(run.status.success(), "{stem}: ngspice failed");
    let bytes = std::fs::read(&out).unwrap_or_else(|e| panic!("{stem}: no raw file: {e}"));
    let text = String::from_utf8_lossy(&bytes);
    text.lines()
        .filter(|l| !l.starts_with("Date:"))
        .collect::<Vec<_>>()
        .join("\n")
        .into_bytes()
}
