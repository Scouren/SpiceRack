//! Outputs as items: what is saved, what is printed, and what is measured. A reader's `.save`,
//! `.probe`, `save`, `.print`/`.plot` and `.meas` in, rows out; the probes they name are rows of
//! `probe.rs`, a measure's typed body a row of `measure.rs`.
//!
//! The save set is always explicit (`INV-semantics.md` 9.03–9.05): a reader expands its
//! simulator's default (ngspice: every node when there is no `.save`; HSPICE: nothing without
//! `.option post`; LTspice: top-level nodes and device currents) and its `all` into `Save` rows
//! of categories, and the union of the `Save` items in force is the set; no later card erases a
//! category. A state checkpoint (HSPICE `.save type=nodeset`, Xyce `.SAVE`, `.DCOP output=`) is
//! not a save: it stays `Opaque` of class `Private`.
//!
//! Cites: `rewrite/IR-RUN.md` §4; `rewrite/INV-constructs.md` H1–H5, H10, H22, H28;
//! `rewrite/INV-semantics.md` 9.03, 9.04, 9.07, 9.08.
//!
//! IR text: `save [analysis=<kind>] <probe> …` (no `analysis=` when it is for every
//! analysis); `print <kind> [hb_time] <probe> …`; measures: `measure_facts.rs`.
//!
//! Contents: `pub struct Save`, `pub struct Print`, `pub enum PrintView`, `pub struct Measure`;
//! `PrintView::ALL`, `PrintView::COUNT`; `RowFacts` for `Save` and `Print` (`Measure`'s:
//! `measure_facts.rs`)

use crate::analysis::AnalysisKind;
use crate::analysis_facts::report;
use crate::build::Builder;
use crate::check::Violation;
use crate::design::Design;
use crate::diag::Diags;
use crate::facts::{Handle, RowFacts};
use crate::feature::FeatureSet;
#[cfg(any(test, feature = "gen"))]
use crate::r#gen::Gen;
use crate::id::{ItemRef, NameId, Ref, ScopeId};
use crate::item::DefKind;
use crate::measure::MeasureKind;
#[cfg(any(test, feature = "gen"))]
use crate::probe::gen_probe;
use crate::probe::{
    Probe, ProbeList, dump_probe, parse_probe, probe_features, probe_handles, probe_valid,
};
use crate::run_extra::RunExtra;
#[cfg(any(test, feature = "gen"))]
use crate::run_words::has;
use crate::run_words::{
    flag, get, put_flag, put_word, take_enum, take_key, take_word, trim_lead, word,
};
#[cfg(any(test, feature = "gen"))]
use crate::span::Span;
use crate::text::Words;

/// Quantities to keep in the results: a member of the deck's save set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Save {
    /// The analysis kind whose results keep them (HSPICE `.probe tran …`); read only when
    /// `every_analysis` is false.
    pub analysis: AnalysisKind,
    /// Whether they are kept for every analysis (`.save`, Spectre and VACASK `save`).
    pub every_analysis: bool,
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

const _: () = assert!(size_of::<Save>() == 12);
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

    fn map_refs(&mut self, _f: &mut dyn FnMut(&mut Ref, DefKind)) {}

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        flag(set, !self.every_analysis, RunExtra::SavePerAnalysis);
        list_features(design, self.probes, set);
    }

    fn check(&self, design: &Design, at: ItemRef, out: &mut Diags) {
        if let Some(v) = list_fault(design, self.probes) {
            report(design, at, v, out);
        }
    }

    fn dump(&self, design: &Design, out: &mut String) {
        let at = out.len();
        if !self.every_analysis {
            put_word(out, &format!("analysis={}", word(self.analysis)));
        }
        dump_list(design, self.probes, out);
        trim_lead(out, at);
    }

    fn parse(
        words: &mut Words<'_>,
        scope: ScopeId,
        b: &mut Builder<'_>,
        diags: &mut Diags,
    ) -> ItemRef {
        let (mut analysis, mut every_analysis) = (AnalysisKind::Op, true);
        if take_key(words, "analysis") {
            let Some(kind) = take_enum(words, &AnalysisKind::ALL, diags) else {
                return ItemRef::NONE;
            };
            (analysis, every_analysis) = (kind, false);
        }
        let Some(probes) = parse_list(words, scope, b, diags) else {
            return ItemRef::NONE;
        };
        b.push(
            scope,
            Save {
                analysis,
                every_analysis,
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
        let probes = gen_list(g, b, features);
        let per = has(features, RunExtra::SavePerAnalysis) && g.chance(3);
        let analysis = if per {
            AnalysisKind::ALL[g.below(AnalysisKind::COUNT as u32) as usize]
        } else {
            AnalysisKind::Op
        };
        b.push(
            scope,
            Save {
                analysis,
                every_analysis: !per,
                probes,
            },
            Span::NONE,
        )
    }
}

impl RowFacts for Print {
    fn handles(&self, design: &Design, out: &mut Vec<Handle>) {
        list_handles(design, self.probes, out);
    }

    fn map_refs(&mut self, _f: &mut dyn FnMut(&mut Ref, DefKind)) {}

    fn features(&self, design: &Design, set: &mut FeatureSet) {
        flag(set, self.view == PrintView::HbTime, RunExtra::PrintHbTime);
        list_features(design, self.probes, set);
    }

    fn check(&self, design: &Design, at: ItemRef, out: &mut Diags) {
        if let Some(v) = list_fault(design, self.probes) {
            report(design, at, v, out);
        }
    }

    fn dump(&self, design: &Design, out: &mut String) {
        out.push_str(&word(self.analysis));
        put_flag(out, self.view == PrintView::HbTime, "hb_time");
        dump_list(design, self.probes, out);
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

fn dump_list(design: &Design, list: ProbeList, out: &mut String) {
    for p in design.probes(list) {
        dump_probe(design, p, out);
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
