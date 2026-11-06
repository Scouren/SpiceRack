//! The built-in languages, each declared with the public definition API
//! (`syntax::def`) in a file of its own, registered in a fixed order on
//! the first read of the registry (`lang`): a definition that extends or
//! names another (ESPice extends ngspice, Spectre's `spice` region is
//! HSPICE's) comes after it.
//!
//! The built-ins not yet declared here are registered from their old
//! tables (`from_tables`) until they move (DIALECT-PLUGIN.md, P4 to P7).
//!
//! Cites: none (each definition cites its manual).
//!
//! Contents:
//! - `pub(crate) fn register_builtins()`

mod ngspice;

use crate::syntax::behaviour;
use crate::syntax::def::register;
use crate::syntax::lang::{self, CONSTRUCT_COUNT, CONSTRUCTS, Row};
use crate::syntax::lexicon;
use crate::syntax::render::capability::{Action, action};
use crate::syntax::render::spelling::spelling;
use crate::syntax::{Dialect, Lang};

/// Registers every built-in, at its constant handle.
///
/// # Panics
/// When a built-in's definition does not register, or lands at another
/// handle than its constant.
pub(crate) fn register_builtins() {
    let order = [
        Dialect::Ngspice,
        Dialect::Espice,
        Dialect::Hspice,
        Dialect::Xyce,
        Dialect::Ltspice,
        Dialect::Spectre,
        Dialect::Vacask,
        Dialect::Cdl,
        Dialect::Verilog,
    ];
    for d in order {
        let got = match d {
            Dialect::Ngspice => register(ngspice::ngspice()).expect("ngspice registers"),
            _ => lang::append(|_| from_tables(d)).expect("a slot for a built-in"),
        };
        assert_eq!(got, Lang::from(d), "{d:?} at its handle");
    }
}

// ---- private ----

/// Built-in `d`'s row, from its old per-dialect tables.
fn from_tables(d: Dialect) -> Row {
    let lexicon = match d {
        Dialect::Ngspice | Dialect::Espice => Some(&lexicon::NGSPICE),
        Dialect::Hspice => Some(&lexicon::HSPICE),
        Dialect::Ltspice => Some(&lexicon::LTSPICE),
        Dialect::Xyce => Some(&lexicon::XYCE),
        Dialect::Spectre => Some(&lexicon::SPECTRE),
        Dialect::Vacask => Some(&lexicon::VACASK),
        Dialect::Cdl | Dialect::Verilog => None,
    };
    let mut cells = [Action::Native; CONSTRUCT_COUNT];
    for (i, c) in CONSTRUCTS.into_iter().enumerate() {
        cells[i] = action(d, c);
    }
    Row {
        name: d.name(),
        lexicon,
        spelling: spelling(d),
        cells,
        behaviours: behaviour::builtin(d),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::syntax::lexicon::Lexicon;

    #[test]
    fn each_built_in_row_is_its_old_tables() {
        // The oracle is the per-dialect tables the writers read before the
        // definitions (the lexicon const, the spelling row, the capability
        // match, the behaviour column): a built-in declared with the API
        // registers the same row, field by field, its tables read as the
        // reader reads them (the first row holding a word).
        for d in Dialect::ALL {
            let (got, want) = (Lang::from(d), from_tables(d));
            assert_eq!(got.name(), want.name);
            match (got.lexicon(), want.lexicon) {
                (Some(a), Some(b)) => same_lexicon(a, b, d),
                (None, None) => {}
                (a, b) => panic!("{d:?}: lexicon {} vs {}", a.is_some(), b.is_some()),
            }
            let (a, b) = (got.spelling(), want.spelling);
            assert_eq!(a.lexicon.lang, b.lexicon.lang, "{d:?}");
            assert_eq!(a.comment, b.comment, "{d:?}");
            assert_eq!(a.path_quote, b.path_quote, "{d:?}");
            assert_eq!(a.expr, b.expr, "{d:?}");
            assert_eq!(a.func, b.func, "{d:?}");
            assert_eq!(a.subckt_params, b.subckt_params, "{d:?}");
            assert_eq!(a.ngspice_numbers, b.ngspice_numbers, "{d:?}");
            for c in CONSTRUCTS {
                assert_eq!(got.action(c), want.cells[c as usize], "{d:?} {c:?}");
            }
            assert_eq!(*got.behaviours(), want.behaviours, "{d:?}");
        }
    }

    /// `a` reads text as `b` does: every field, its tables flattened to the
    /// rows a lookup finds.
    fn same_lexicon(a: &Lexicon, b: &Lexicon, d: Dialect) {
        let first = |rows: Vec<(&'static str, String)>| {
            let mut out: Vec<(&str, String)> = Vec::new();
            for r in rows {
                if !out.iter().any(|o| o.0 == r.0) {
                    out.push(r);
                }
            }
            out
        };
        let kw = |l: &Lexicon| {
            first(
                l.keywords
                    .iter()
                    .flat_map(|t| t.iter())
                    .map(|r| (r.0, format!("{:?}", r.1)))
                    .collect(),
            )
        };
        let dev = |l: &Lexicon| {
            l.devices
                .iter()
                .flat_map(|t| t.iter())
                .map(|r| format!("{r:?}"))
                .collect::<Vec<_>>()
        };
        assert_eq!(a.lang, b.lang, "{d:?} lang");
        assert_eq!(
            (
                a.title,
                a.fold_case,
                a.comment_lines,
                a.comment_col1,
                a.blank_led_comment,
                a.cont_skip,
                a.comment_tails
            ),
            (
                b.title,
                b.fold_case,
                b.comment_lines,
                b.comment_col1,
                b.blank_led_comment,
                b.cont_skip,
                b.comment_tails
            ),
            "{d:?} lines"
        );
        assert_eq!(
            (
                a.block_comment,
                a.long_strings,
                a.dollar,
                a.plus,
                a.backslash,
                a.end,
                a.stop,
                a.join
            ),
            (
                b.block_comment,
                b.long_strings,
                b.dollar,
                b.plus,
                b.backslash,
                b.end,
                b.stop,
                b.join
            ),
            "{d:?} breaks"
        );
        assert_eq!(
            (
                a.quote,
                a.string,
                a.string_expr,
                a.name_escape,
                a.ident_start,
                a.ident_continue,
                a.directive,
                a.head_words,
                a.heads_after
            ),
            (
                b.quote,
                b.string,
                b.string_expr,
                b.name_escape,
                b.ident_start,
                b.ident_continue,
                b.directive,
                b.head_words,
                b.heads_after
            ),
            "{d:?} words"
        );
        assert_eq!(rows(a.operators), rows(b.operators), "{d:?} operators");
        assert_eq!(format!("{:?}", a.ops), format!("{:?}", b.ops), "{d:?} ops");
        assert_eq!(
            (a.expr_braces, a.caret, a.lib_sections, a.named),
            (b.expr_braces, b.caret, b.lib_sections, b.named),
            "{d:?}"
        );
        assert_eq!(
            (a.numbers.exponent, a.numbers.tail, a.numbers.arithmetic),
            (b.numbers.exponent, b.numbers.tail, b.numbers.arithmetic),
            "{d:?} numbers"
        );
        assert_eq!(
            rows(a.numbers.scales),
            rows(b.numbers.scales),
            "{d:?} scales"
        );
        assert_eq!(kw(a), kw(b), "{d:?} keywords");
        assert_eq!(dev(a), dev(b), "{d:?} devices");
        assert_eq!(rows(a.masters), rows(b.masters), "{d:?} masters");
        assert_eq!(rows(a.analyses), rows(b.analyses), "{d:?} analyses");
        let regions = |l: &Lexicon| {
            l.regions
                .iter()
                .map(|r| (r.0, r.1.lang))
                .collect::<Vec<_>>()
        };
        assert_eq!(regions(a), regions(b), "{d:?} regions");
    }

    /// `v`'s rows as a lookup finds them: the first holding each key.
    fn rows<T: std::fmt::Debug>(v: &[(&'static str, T)]) -> Vec<(&'static str, String)> {
        let mut out: Vec<(&'static str, String)> = Vec::new();
        for r in v {
            if !out.iter().any(|o| o.0 == r.0) {
                out.push((r.0, format!("{:?}", r.1)));
            }
        }
        out
    }
}
