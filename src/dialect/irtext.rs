//! The IR's own text as a dialect ("dialect zero", name `ir`, suffix `.ir`): IR text in →
//! `ir::parse` per file; a design's file → `ir::dump`. Its writer spells every feature and
//! every dialect's `Opaque`, since the text is lossless. It makes every stage runnable from
//! the command line (`spice2rack --dialect ir`, `--to ir`, `--dump-ir`) and is the first row
//! the dialect laws run on.
//!
//! Cites: `rewrite/SPINE.md` §6; `REWRITE-PLAN.md` §3.6; `crates/ir/src/text.rs` (the grammar).
//!
//! Contents: `pub struct IrText` (`impl Read`, `impl Write`), `pub const SUFFIX: &str`

use std::sync::OnceLock;

use super::{Read, Reference, Sources, Write};
use crate::ir;

/// The `ir` row's reader and writer.
#[derive(Clone, Copy, Debug)]
pub struct IrText;

/// The file suffix of IR text.
pub const SUFFIX: &str = "ir";

impl Read for IrText {
    /// The `include "path"` lines (`Reference::File`) and `load veriloga|osdi "path"` lines
    /// (`Reference::VerilogA`), in text order. A line whose string does not close is skipped:
    /// `read` reports it.
    fn references(&self, text: &str) -> Vec<Reference> {
        let mut out = Vec::new();
        for line in text.lines() {
            let line = line.trim_start();
            if let Some(rest) = line.strip_prefix("include ") {
                out.extend(string(rest).map(Reference::File));
            } else if let Some(rest) = line.strip_prefix("load ") {
                let rest = rest.trim_start();
                let rest = rest.split_once(' ').map_or("", |(_, r)| r);
                out.extend(string(rest).map(Reference::VerilogA));
            }
        }
        out
    }

    /// Each file's text through `ir::parse`, in `src`'s order; the text's own `file` lines name
    /// the design's files.
    fn read(&self, src: &Sources, out: &mut ir::Builder<'_>, diags: &mut ir::Diags) {
        assert!(!src.files().is_empty(), "IrText::read: a file to read");
        for (_, text) in src.files() {
            ir::parse(text, out, diags);
        }
    }
}

impl Write for IrText {
    /// Every feature.
    fn features(&self) -> &'static ir::FeatureSet {
        static ALL: OnceLock<ir::FeatureSet> = OnceLock::new();
        ALL.get_or_init(|| {
            (0..ir::Feature::COUNT).fold(ir::FeatureSet::EMPTY, |set, i| {
                set.with(ir::Feature::from_index(i))
            })
        })
    }

    /// `ir::dump` of `file`: its `file` line and its items.
    fn write(
        &self,
        design: &ir::Design,
        file: ir::FileId,
        out: &mut String,
        _diags: &mut ir::Diags,
    ) {
        let before = out.len();
        ir::dump(design, file, out);
        assert!(
            out.len() > before,
            "IrText::write: a file dumps at least its `file` line"
        );
    }
}

// ---- private ----

/// The contents of the IR-text string that opens `rest` (after blanks), its escapes undone;
/// `None` when it does not open or close one.
fn string(rest: &str) -> Option<String> {
    let mut chars = rest.trim_start().strip_prefix('"')?.chars();
    let mut out = String::new();
    // Bounded by the line's length.
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                'n' => out.push('\n'),
                c => out.push(c),
            },
            c => out.push(c),
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_the_include_and_load_lines_in_order() {
        let text = "file \"a.ir\" deck\n  include \"m/x.ir\" section=tt\n  title \"include \\\"no\\\"\"\n    load veriloga \"va/r.va\" modules=(r)\n  include \"q\\\"z.ir\"\n  include \"open\n";
        assert_eq!(
            IrText.references(text),
            [
                Reference::File("m/x.ir".into()),
                Reference::VerilogA("va/r.va".into()),
                Reference::File("q\"z.ir".into()),
            ]
        );
    }
}
