//! Outputs as items: what is saved, what is printed, and what is measured. A reader's `.save`,
//! `.probe`, `save`, `.print`/`.plot` and `.meas` in, rows out; the probes they name are rows of
//! `probe.rs`, a measure's typed body a row of `measure.rs`.
//!
//! The save set is the union of the `Save` items in force; a reader expands its simulator's
//! `all` into `Save` rows of categories (ngspice: `AllV` and `AllBranchI`), and no later card
//! erases a category (`INV-semantics.md` 9.04, 9.05). What a deck saves when it names nothing is
//! not stated: today the IR is silent and the target's default applies (R29, open until the run
//! side, S12). A state checkpoint (HSPICE `.save type=nodeset`, Xyce `.SAVE`, `.DCOP output=`) is
//! not a save: it stays `Opaque` of class `Private`.
//!
//! Cites: `rewrite/IR-RUN.md` §4; `rewrite/INV-constructs.md` H1–H5, H10, H22, H28;
//! `rewrite/INV-semantics.md` 9.03, 9.04, 9.07, 9.08.
//!
//! IR text: `save [analysis=<kind>|analysis=(<kind> …)] [from_here] <probe> …` (no `analysis=`
//! when it is for every analysis; one kind bare, several in declaration order in parentheses;
//! `from_here` for `Applies::FromHere`, A27); `print <kind> [hb_time] <probe> …`; measures: `measure_facts.rs`.
//!
//! Contents: `pub struct Save`, `pub struct Print`, `pub enum PrintView`, `pub struct Measure`;
//! `PrintView::ALL`, `PrintView::COUNT`; `RowFacts` for `Save` and `Print` (`Measure`'s:
//! `measure_facts.rs`)

use crate::analysis::{AnalysisKind, AnalysisKinds};
use crate::analysis_facts::report;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{ExprUse, Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ExprId, ItemRef, NameId, Ref, ScopeId};
use crate::item::DefKind;
use crate::measure::MeasureKind;
use crate::option::Applies;
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{
    Probe, ProbeList, dump_probe, parse_probe, probe_features, probe_handles, probe_valid,
};
use crate::run_exprs::roots;
use crate::run_extra::RunExtra;
use crate::run_names::names;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    close, flag, get, open, put_flag, put_word, take_enum, take_key, take_word, trim_lead, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;
use crate::text_expr::Ctx;

/// Quantities to keep in the results: a member of the deck's save set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Save {
    /// The analyses whose results keep them, exactly: `AnalysisKinds::ALL` for every analysis
    /// (`.save`, Spectre `save`), one kind (HSPICE `.probe tran …`), or a set (VACASK's `save`
    /// governs the operating point, its sweeps and the transient, not AC; A22). Never empty
    /// (checked).
    pub analyses: AnalysisKinds,
    /// Where it holds from (A27): `WholeRun` for a SPICE `.save` card (every analysis of the
    /// run, wherever the card stands; the builder's default), `FromHere` for VACASK's `save`
    /// and an ngspice script `save` (the analyses after it in run order; saves add up, and a
    /// `Reset` of `Saves` drops the earlier `FromHere` ones).
    pub holds: Applies,
    /// The quantities, at least one.
    pub probes: ProbeList,
}

/// Quantities to print as a table for one analysis kind (`.print tran v(1)`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Print {
    /// The analysis kind the table belongs to.
    pub analysis: AnalysisKind,
    /// Which view of that analysis.
    pub view: PrintView,
    /// The columns, in order, each a separate probe (ngspice reads `print a -b` as `a − b`;
    /// that reader builds one `Expr` probe, `INV-semantics.md` 9.07).
    pub probes: ProbeList,
}

/// Which view of an analysis a print shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum PrintView {
    /// The analysis's own result (for HB: the spectrum, Xyce `HB_FD`, HSPICE `HB`).
    Main,
    /// HB's time-domain waveform (Xyce `HB_TD`, HSPICE `HBTRAN`).
    HbTime,
}

/// A named measurement over one analysis kind's results. Its body is `detail` in `kind`'s table
/// (`Design::detail`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measure {
    /// The result's name, read by later measures.
    pub name: NameId,
    /// The analysis kind whose results it reads (`.meas tran …`).
    pub analysis: AnalysisKind,
    /// What it computes; names the detail table.
    pub kind: MeasureKind,
    /// Row in the kind's detail table; always present.
    pub detail: u32,
}

const _: () = assert!(size_of::<Save>() == 16);
const _: () = assert!(size_of::<Print>() == 12);
const _: () = assert!(size_of::<Measure>() == 12);

impl PrintView {
    /// Every view, in declaration order.
    pub const ALL: [PrintView; 2] = [PrintView::Main, PrintView::HbTime];
    /// View count.
    pub const COUNT: usize = Self::ALL.len();
}

impl RowFacts for Save {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        list_handles(design, self.probes, out);
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let row: Save = design.row(at.index);
        roots(design, f).probes(row.probes);
    }

    fn map_element_names(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut [NameId])) {
        let row: Save = design.row(at.index);
        names(design, f).probes(row.probes);
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let _ = (design, at, f); // no reference
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        let n = self.analyses.len();
        flag(set, n == 1, RunExtra::SavePerAnalysis);
        flag(
            set,
            n > 1 && self.analyses != AnalysisKinds::ALL,
            RunExtra::SaveAnalysisSet,
        );
        flag(set, true, save_mode(self.holds));
        list_features(design, self.probes, set);
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if let Some(v) = list_fault(design, self.probes) {
            report(design, at, v, out);
        }
        if self.analyses.is_empty() {
            report(design, at, Violation::FieldDomain, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        let at = out.len();
        if self.analyses != AnalysisKinds::ALL {
            let kinds: Vec<String> = self.analyses.iter().map(word).collect();
            let kinds = match kinds.as_slice() {
                [one] => one.clone(),
                _ => format!("({})", kinds.join(" ")),
            };
            put_word(out, &format!("analysis={kinds}"));
        }
        put_flag(out, self.holds == Applies::FromHere, "from_here");
        dump_list(design, ctx, self.probes, out);
        trim_lead(out, at);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let mut analyses = AnalysisKinds::ALL;
        if take_key(words, "analysis") {
            let several = words.peek() == Some("(");
            if several && !open(words, diags) {
                return ItemRef::NONE;
            }
            let mut kinds = Vec::new();
            // one kind per pass; the bound allows each kind twice (repeats are one member)
            for _ in 0..2 * AnalysisKind::COUNT {
                let Some(kind) = take_enum(words, &AnalysisKind::ALL, diags) else {
                    return ItemRef::NONE;
                };
                kinds.push(kind);
                if !several || words.peek() == Some(")") {
                    break;
                }
            }
            if several && !close(words, diags) {
                return ItemRef::NONE;
            }
            analyses = AnalysisKinds::of(&kinds);
        }
        let holds = if take_word(words, "from_here") {
            Applies::FromHere
        } else {
            Applies::WholeRun
        };
        let Some(probes) = parse_list(words, scope, b, diags) else {
            return ItemRef::NONE;
        };
        let row = Save {
            analyses,
            holds,
            probes,
        };
        b.push(scope, row, words.span())
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let Some(holds) =
            crate::state_gen::gen_holds(g, b, scope, features, RunExtra::SaveMidRun, save_mode)
        else {
            return ItemRef::NONE;
        };
        let probes = gen_list(g, b, features);
        let n = AnalysisKind::COUNT as u32;
        let (a, c) = (g.below(n), g.below(n - 1));
        let first = AnalysisKind::ALL[a as usize];
        let second = AnalysisKind::ALL[((a + 1 + c) % n) as usize]; // any other kind
        let analyses = match g.below(3) {
            0 if has(features, RunExtra::SavePerAnalysis) => AnalysisKinds::of(&[first]),
            1 if has(features, RunExtra::SaveAnalysisSet) => AnalysisKinds::of(&[first, second]),
            _ => AnalysisKinds::ALL,
        };
        let row = Save {
            analyses,
            holds,
            probes,
        };
        b.push(scope, row, Span::NONE)
    }
}

impl RowFacts for Print {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        list_handles(design, self.probes, out);
    }

    fn map_exprs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut ExprId, ExprUse)) {
        let row: Print = design.row(at.index);
        roots(design, f).probes(row.probes);
    }

    fn map_element_names(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut [NameId])) {
        let row: Print = design.row(at.index);
        names(design, f).probes(row.probes);
    }

    fn map_refs(design: &mut Design, at: ItemRef, f: &mut dyn FnMut(&mut Ref, DefKind)) {
        let _ = (design, at, f); // no reference
    }

    fn refs(&self, _design: &Design, _at: ItemRef, _out: &mut Vec<(Ref, DefKind)>) {
        // no reference field
    }

    fn features(&self, design: &Design, _at: ItemRef, _scope: ScopeId, set: &mut FeatureSet) {
        flag(set, self.view == PrintView::HbTime, RunExtra::PrintHbTime);
        list_features(design, self.probes, set);
    }

    fn check(&self, design: &Design, at: ItemRef, _scope: ScopeId, out: &mut Diags) {
        if let Some(v) = list_fault(design, self.probes) {
            report(design, at, v, out);
        }
    }

    fn dump(&self, design: &Design, ctx: &Ctx, out: &mut String) {
        out.push_str(&word(self.analysis));
        put_flag(out, self.view == PrintView::HbTime, "hb_time");
        dump_list(design, ctx, self.probes, out);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let Some(analysis) = take_enum(words, &AnalysisKind::ALL, diags) else {
            return ItemRef::NONE;
        };
        let view = if take_word(words, "hb_time") {
            PrintView::HbTime
        } else {
            PrintView::Main
        };
        let Some(probes) = parse_list(words, scope, b, diags) else {
            return ItemRef::NONE;
        };
        b.push(
            scope,
            Print {
                analysis,
                view,
                probes,
            },
            words.span(),
        )
    }

    #[cfg(any(test, feature = "gen"))]
    fn generate(
        g: &mut Gen,
        scope: ScopeId,
        b: &mut Builder<'_>,
        features: &FeatureSet,
    ) -> ItemRef {
        if !crate::state_order::run_append_ok(b.design(), scope, features) {
            return ItemRef::NONE; // no `RunInterleaved` outside the set (A22)
        }
        if !features.contains(crate::feature::Feature::Expr(crate::expr::ExprTag::Num)) {
            return ItemRef::NONE; // every run row's fields need literals (`Gen::expr`)
        }
        let probes = gen_list(g, b, features);
        let analysis = AnalysisKind::ALL[g.below(AnalysisKind::COUNT as u32) as usize];
        let hb = has(features, RunExtra::PrintHbTime) && g.chance(3);
        let view = if hb {
            PrintView::HbTime
        } else {
            PrintView::Main
        };
        b.push(
            scope,
            Print {
                analysis,
                view,
                probes,
            },
            Span::NONE,
        )
    }
}

// ---- private ----

fn list_handles(design: &Design, list: ProbeList, out: &mut Vec<Handle>) {
    for p in get(&design.run.probes, list).unwrap_or(&[]) {
        probe_handles(p, out);
    }
}

fn list_features(design: &Design, list: ProbeList, set: &mut FeatureSet) {
    for p in get(&design.run.probes, list).unwrap_or(&[]) {
        probe_features(p, set);
    }
}

/// `HandleRange` for a list past its pool, `FieldDomain` for an empty list or a bad probe.
fn list_fault(design: &Design, list: ProbeList) -> Option<Violation> {
    let Some(probes) = get(&design.run.probes, list) else {
        return Some(Violation::HandleRange);
    };
    (probes.is_empty() || !probes.iter().all(probe_valid)).then_some(Violation::FieldDomain)
}

fn dump_list(design: &Design, ctx: &Ctx, list: ProbeList, out: &mut String) {
    for p in design.probes(list) {
        dump_probe(design, ctx, p, out);
    }
}

fn parse_list(
    words: &mut Words<'_>,
    scope: ScopeId,
    b: &mut Builder<'_>,
    diags: &mut Diags,
) -> Option<ProbeList> {
    let mut probes: Vec<Probe> = Vec::new();
    while words.peek() == Some("(") {
        probes.push(parse_probe(words, scope, b, diags)?);
    }
    Some(b.probes(&probes))
}

#[cfg(any(test, feature = "gen"))]
fn gen_list(g: &mut Gen, b: &mut Builder<'_>, features: &FeatureSet) -> ProbeList {
    let n = 1 + g.below(2) as usize;
    let probes: Vec<Probe> = (0..n).map(|_| gen_probe(g, b, features, false)).collect();
    b.probes(&probes)
}

/// The feature of a save's mode (A27).
const fn save_mode(holds: Applies) -> RunExtra {
    match holds {
        Applies::FromHere => RunExtra::SaveFromHere,
        Applies::WholeRun => RunExtra::SaveWholeRun,
    }
}
