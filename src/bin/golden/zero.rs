//! A golden record -> the Z1 list: every output that carries `W0001` (a
//! statement kept as another dialect's text) or `W0032` (text that did not
//! parse), by name, strict against the checked-in list.
//!
//! A name is `<set>/<relpath> <target>`, a `migration.xfail` key. An output
//! carries a code on the code line of its `<target>.refused` or
//! `<target>.notes` (`codes: W0001 W0013`) or, for CDL, in an `* error:`
//! comment of the written deck (an element CDL has no form for leaves as
//! one, `render/spice/cdl.rs`). The record's `MANIFEST` names every output,
//! so no directory is walked.
//!
//! `tests/fixtures/zero/untranslated.txt` is the list: the names, sorted by
//! bytes, one per line. It only shrinks. A name the record has and the list
//! lacks is a new untranslated output (`NEW`); a listed name the record no
//! longer has was cleared, and its line goes (`CLEARED`). Either fails.
//!
//! Cites: ZERO-LEDGER P1-029 (gate Z1: no output carries W0001 or W0032);
//! VerA `tools/vera.xfail:1-5` (strict lists).
//!
//! Contents:
//! - `pub(super) fn zero(record: &Path, list: Option<&Path>) -> std::io::Result<bool>`

use std::collections::BTreeSet;
use std::path::Path;

use super::diff::CODES;

/// The codes of an untranslated output: text kept as another dialect's, and
/// text that did not parse.
const UNTRANSLATED: [&str; 2] = ["W0001", "W0032"];

/// A CDL deck's comment for an element it has no form for starts so, the
/// refusing codes and their titles after it.
const CDL_ERROR: &str = "* error: ";

/// Prints the Z1 names of `record`, one per line (no `list`: what the list
/// file holds), or compares them with `list` and prints each difference.
/// `Ok(true)` when printed, or when the two hold the same names.
pub(super) fn zero(record: &Path, list: Option<&Path>) -> std::io::Result<bool> {
    let got = names(record)?;
    let Some(list) = list else {
        got.iter().for_each(|n| println!("{n}"));
        return Ok(true);
    };
    let text = std::fs::read_to_string(list)?;
    let want: BTreeSet<String> = text.lines().map(str::to_string).collect();
    for n in got.difference(&want) {
        println!("NEW {n}: carries W0001 or W0032 and is not listed");
    }
    for n in want.difference(&got) {
        println!("CLEARED {n}: listed, and carries neither now (remove the line)");
    }
    let same = got == want;
    if same {
        println!("Z1: {} untranslated outputs, as listed", got.len());
    }
    Ok(same)
}

// ---- private ----

/// The names of `record`'s outputs that carry one of `UNTRANSLATED`, from
/// its `MANIFEST` (`output <set>/<relpath>/<target>.<ext> <hash>`). An
/// output line that names no such file is an error, never passed over.
fn names(record: &Path) -> std::io::Result<BTreeSet<String>> {
    let manifest = std::fs::read_to_string(record.join("MANIFEST"))?;
    let bad = |l: &str| std::io::Error::other(format!("MANIFEST: no output file in `{l}`"));
    let mut out = BTreeSet::new();
    for line in manifest.lines() {
        let Some(rest) = line.strip_prefix("output ") else {
            continue;
        };
        // The hash is the last word, so a path may hold spaces.
        let (rel, _) = rest.rsplit_once(' ').ok_or_else(|| bad(line))?;
        let (input, file) = rel.rsplit_once('/').ok_or_else(|| bad(line))?;
        let (target, ext) = file.split_once('.').ok_or_else(|| bad(line))?;
        let coded = matches!(ext, "refused" | "notes");
        if !coded && file != "cdl.txt" {
            continue;
        }
        let text = std::fs::read_to_string(record.join(rel))?;
        if carries(&text, coded) {
            out.insert(format!("{input} {target}"));
        }
    }
    Ok(out)
}

/// Whether an output's text carries one of `UNTRANSLATED`: on its code line
/// (`coded`: a refusal or a notes file), else in a CDL deck's error
/// comments.
fn carries(text: &str, coded: bool) -> bool {
    let has = |l: &str| l.split(' ').any(|w| UNTRANSLATED.contains(&w));
    let first = text.lines().next().unwrap_or("");
    let mut comments = text.lines().filter(|l| l.starts_with(CDL_ERROR));
    match coded {
        true => first.starts_with(CODES) && has(first),
        false => comments.any(has),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A record under a fresh temp directory: `files`, each
    /// `<set>/<relpath>/<target>.<ext>` and its text, and their MANIFEST.
    fn record(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
        let name = format!("golden-zero-{tag}-{}", std::process::id());
        let root = std::env::temp_dir().join(name);
        let mut manifest = vec!["set s s ngspice 1".to_string()];
        for (rel, text) in files {
            let f = root.join(rel);
            std::fs::create_dir_all(f.parent().unwrap()).unwrap();
            std::fs::write(&f, text).unwrap();
            manifest.push(format!("output {rel} 0123456789abcdef"));
        }
        std::fs::write(root.join("MANIFEST"), manifest.join("\n") + "\n").unwrap();
        root
    }

    #[test]
    fn p1_029_a_name_is_an_output_carrying_w0001_or_w0032() {
        // Oracle: gate Z1's definition (ZERO-LEDGER "Zero gates"), on a
        // record written by hand. A coded refusal, a notes file and a CDL
        // error comment carry the codes; another code, a message, another
        // target's comment, a free-text refusal and a path do not.
        let files = [
            ("s/a.sp/ngspice.refused", "codes: W0001 W0013\n"),
            ("s/a.sp/hspice.refused", "codes: W0013\nW0001 said\n"),
            ("s/a.sp/ltspice.txt", "a\n* error: W0001 its own\n"),
            ("s/a.sp/xyce.txt", "a\n"),
            ("s/a.sp/xyce.notes", "codes: P0010 W0032\n"),
            ("s/a b.sp/cdl.txt", "* error: W0013 x; W0032 y\n"),
            ("s/a b.sp/cdl.notes", "codes: W0021\n"),
            ("s/c.scs/parse.refused", "no Cdl reader yet\n"),
            ("s/d.sp/cdl.txt", "* error: no (W0001.va)\n"),
        ];
        let root = record("names", &files);
        let got: Vec<String> = names(&root).unwrap().into_iter().collect();
        assert_eq!(got, ["s/a b.sp cdl", "s/a.sp ngspice", "s/a.sp xyce"]);
        std::fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn p1_029_the_list_is_strict_both_ways() {
        // Oracle: the strict-list rule (an unlisted name fails, and so does
        // a listed one the record no longer has), each seeded by hand.
        let root = record("strict", &[("s/a.sp/ngspice.refused", "codes: W0001\n")]);
        let list = root.join("list.txt");
        std::fs::write(&list, "s/a.sp ngspice\n").unwrap();
        assert!(zero(&root, Some(&list)).unwrap());
        // NEW: the record has it, the list lacks it.
        std::fs::write(&list, "").unwrap();
        assert!(!zero(&root, Some(&list)).unwrap());
        // CLEARED: listed, and the record carries neither code there.
        std::fs::write(&list, "s/a.sp ngspice\ns/b.sp xyce\n").unwrap();
        assert!(!zero(&root, Some(&list)).unwrap());
        // An output line that names no file is an error, never skipped.
        std::fs::write(root.join("MANIFEST"), "output nofile\n").unwrap();
        assert!(zero(&root, Some(&list)).is_err());
        std::fs::remove_dir_all(&root).unwrap();
    }
}
