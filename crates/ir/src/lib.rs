//! SpiceRack's dialect-neutral semantic IR: appends from a reader or a builder in, a `Design`
//! (owned tables of typed rows) out, read by writers and rewritten by passes. Meaning, not
//! spelling: no token, no source text and no dialect tag lives here except in an `Opaque` item.
//!
//! Doors (C13): readers and builders get [`Builder`] (append only), writers get `&`[`Design`]
//! (read only), passes get [`Editor`]. Table fields are crate-private; the three owners of this
//! crate (kernel, circuit, run; `rewrite/IR-KERNEL.md` §10) each add rows and methods in their
//! own files.
//!
//! A caller reads a source into a design, checks it, and writes it. Here the reader and the
//! writer are the crate's own IR text (`parse`, `dump`); a dialect's reader and writer take the
//! same places, and its `FEATURES` the place of `FeatureSet::ALL`.
//!
//! ```
//! use spicerack_ir::{Design, Diags, FeatureSet, check, check_legal, dump, parse};
//!
//! let source = "file \"rc.sp\" deck\n  param rval = 1k\n  param half = rval / 2\n";
//! let mut design = Design::new();
//! let mut diags = Diags::new();
//! parse(source, &mut design.builder(), &mut diags);
//! check(&design, &mut diags);
//! check_legal(&design, &FeatureSet::ALL, &mut diags);
//! assert!(diags.is_empty());
//! let mut text = String::new();
//! dump(&design, design.entry(), &mut text);
//! assert_eq!(text, source);
//! ```
//!
//! Cites: `REWRITE-PLAN.md` §2–§4; `rewrite/IR-KERNEL.md` (conventions C1–C14, Addenda A1–A7);
//! `rewrite/RULINGS.md`.
//!
//! Contents: the modules below (kernel, circuit owner, run owner, in three groups), and
//! re-exports:
//! - kernel: `Design`, `ItemRow`, `DetailRow`, `Builder`, `Editor`, the handles of `id` (`NameId`, `FileId`,
//!   `ScopeId`, `ExprId`, `DefId`, `TextId`, `NumId`, `ItemRef`, `Ref`, `List` and its aliases),
//!   `Span`, `Table`, the kernel enums of `item` and `expr`, `Literal`, `NumForm`, `RowFacts`,
//!   `Handle`, `ExprUse`, `Feature`, `FeatureSet`, `Extra`, `features_used`, `link`, `rewrite_exprs`, `check`,
//!   `check_legal`, `Violation`, the diagnostic rows of `diag`, `eval`, `Bindings`, `Domain`,
//!   `dump`, `parse`, `parse_unchecked`, `Mark` (`Builder::mark`, `Builder::truncate`)
//! - circuit owner: `ElemHead`, `CircuitExtra`, `DetailKind`, `ElemKind`, `Element`, `NO_ROW`, `Model`,
//!   `ModelFamily`, `Polarity`, `Wave`, `WaveKind`, `WaveRef`, `WaveRow`
//! - run owner: `Analysis`, `AnalysisKind`, `AnalysisKinds`, `Reset`, `ResetSet`, `Restorable`, `Listed`, `listed`, `Setting`, `in_force`, `run_walks`, `RunDetail`, `RunDetails`, `RunExtra`, `Edge`, `Event`,
//!   `EventKind`, `MeasureKind`, `Stat`, `Env`, `IcScope`, `NodeSet`, `NodeSetKind`,
//!   `OptionRow`, `OptionClass`, `OptionKey`, `Measure`, `Print`, `PrintView`, `Save`, `Alter`,
//!   `Loop`, `LoopEnd`, `LoopKind`, `Target`, `TargetKind`, `TargetList`, `AcPart`, `Probe`,
//!   `ProbeKind`, `ProbeList`, `Statistics`, `Vary`, `VaryDist`, `SweepBounds`, `SweepEnd`,
//!   `SweepKind`, `SweepSpec`, `SweepCount`, `on_grid`

#![forbid(unsafe_code)]
#![warn(missing_docs)]

// first: its `for_kind!` macro is textually scoped to the modules declared after it
#[macro_use]
pub mod facts;

pub mod build;
pub mod build_expr;
mod case;
pub mod check;
mod check_legal;
mod check_scopes;
pub mod class;
pub mod design;
pub mod diag;
pub mod edit;
pub mod element_uses;
pub mod eval;
pub mod expr;
pub mod feature;
pub mod form;
#[cfg(any(test, feature = "gen"))]
mod gen_expr;
pub mod id;
pub mod item;
pub mod item_facts;
mod item_facts_defs;
mod item_facts_scopes;
pub mod item_text;
pub mod link;
pub mod literal;
mod live;
pub mod mark;
pub mod names;
pub mod rewrite;
pub mod scope;
pub mod span;
pub mod table;
pub mod text;
pub mod text_expr;
pub mod text_lex;
pub mod text_parse;
pub mod text_words;

// circuit owner
pub mod build_circuit;
mod circuit_check;
pub mod circuit_extra;
mod circuit_features;
mod circuit_fields;
#[cfg(any(test, feature = "gen"))]
mod circuit_gen;
mod circuit_parse;
mod circuit_tables;
mod circuit_text;
pub mod controlled;
pub mod element;
mod hierarchy;
pub mod laws;
mod laws_carry;
mod laws_table;
pub mod letter;
pub mod library;
pub mod line;
pub mod model;
pub mod passive;
pub mod semi;
pub mod source;
pub mod wave;
pub mod wave_eval;

pub use build_circuit::ElemHead;
pub use circuit_extra::CircuitExtra;
pub use element::{DetailKind, ElemKind, Element, MultLaw, NO_ROW};
pub use laws::{LTSPICE_EXP_KNEE, Law, Laws, Rewrite, Stated};
pub use letter::{spice_letter, spice_letters};
pub use library::DeviceLibrary;
pub use model::{Model, ModelFamily, Polarity};
pub use wave::{Wave, WaveKind, WaveRef, WaveRow};
pub use wave_eval::{At, WaveDomain, source_value, wave_value};

// run owner
#[macro_use]
pub mod analysis;
// before the detail rows' facts: its `run_fields!`, `run_choices!`
#[macro_use]
mod run_fields;
mod analysis_facts;
pub mod build_run;
pub mod dc_ac;
mod dc_ac_facts;
mod loop_facts;
pub mod measure;
mod measure_facts;
mod network_facts;
pub mod option;
pub mod option_key;
pub mod output;
pub mod periodic;
mod periodic_facts;
mod periodic_ss_facts;
pub mod plan;
mod plan_facts;
pub mod probe;
pub mod reset;
mod run_exprs;
pub mod run_extra;
mod run_names;
mod run_words;
#[cfg(any(test, feature = "gen"))]
mod state_gen;
mod state_order;
pub mod stats;
pub mod sweep;
pub mod tran;
mod xf_facts;

pub use analysis::{Analysis, AnalysisKind, AnalysisKinds, RunDetail, RunDetails};
pub use build::Builder;
pub use check::{Violation, check, check_legal};
pub use class::{ExprClass, expr_class};
pub use design::{Design, DetailRow, ItemRow};
pub use diag::{Code, Diag, Diags, Info, Kind};
pub use edit::Editor;
pub use element_uses::ElementUse;
pub use eval::{Bindings, Domain, Value, eval, eval_typed};
pub use expr::{BinOp, Dist, DrawPer, DrawWhen, Expr, ExprTag, Func, Literal, NumForm, UnOp};
pub use facts::{ExprUse, Handle, RowFacts};
pub use feature::{Extra, Feature, FeatureSet, Form, features_used, root_form};
pub use id::{
    BranchList, DefId, ExprId, ExprList, FileId, ItemRef, List, NameId, NameList, NumId, ParamList,
    Ref, ScopeId, TextId,
};
pub use item::{DefKind, ItemKind, LoadKind, OpaqueClass, Role, ScopeKind};
pub use link::{UnboundRead, UnboundTarget, link, unbound_reads, unbound_targets};
pub use mark::Mark;
pub use measure::{Edge, Event, EventKind, MeasureKind, Stat};
pub use option::{Applies, Env, IcScope, NodeSet, NodeSetKind, OptionRow};
pub use option_key::{OptionClass, OptionKey};
pub use output::{Measure, Print, PrintView, Save};
pub use plan::{
    Alter, Elaborate, Loop, LoopEnd, LoopKind, RefList, Target, TargetKind, TargetList,
};
pub use probe::{AcPart, Probe, ProbeKind, ProbeList};
pub use reset::{Reset, ResetSet, Restorable};
pub use rewrite::{rewrite_exprs, rewrite_exprs_in, rewrite_tree};
pub use run_extra::RunExtra;
pub use span::Span;
pub use state_order::{Listed, Setting, in_force, listed, run_walks};
pub use stats::{Statistics, Vary, VaryDist};
pub use sweep::{SweepBounds, SweepCount, SweepEnd, SweepKind, SweepSpec, on_grid};
pub use table::Table;
pub use text::{dump, parse, parse_unchecked};

// `gen` is a reserved word in edition 2024; this line is last so check_style.py, which does
// not read the raw identifier as a module, sees no import after it.
#[cfg(any(test, feature = "gen"))]
pub mod r#gen;

// `check`'s fixtures need the crate-private tables; last, after `gen`, for check_style.py
#[cfg(test)]
mod check_tests;
#[cfg(test)]
mod element_uses_tests;
#[cfg(test)]
mod form_tests;
