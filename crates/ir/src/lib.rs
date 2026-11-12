//! SpiceRack's dialect-neutral semantic IR: appends from a reader or a builder in, a `Design`
//! (owned tables of typed rows) out, read by writers and rewritten by passes. Meaning, not
//! spelling: no token, no source text and no dialect tag lives here except in an `Opaque` item.
//!
//! Doors (C13): readers and builders get [`Builder`] (append only), writers get `&`[`Design`]
//! (read only), passes get [`Editor`]. Table fields are crate-private; the three owners of this
//! crate (kernel, circuit, run; `rewrite/IR-KERNEL.md` §10) each add rows and methods in their
//! own files.
//!
//! ```ignore
//! let mut design = Design::new();
//! let mut diags = Diags::new();
//! reader.read(&sources, &mut design.builder(), &mut diags);
//! check(&design, &mut diags);
//! check_legal(&design, &writer_features, &mut diags);
//! writer.write(&design, design.entry(), &mut text, &mut diags);
//! ```
//!
//! Cites: `REWRITE-PLAN.md` §2–§4; `rewrite/IR-KERNEL.md` (conventions C1–C14, Addenda A1–A6);
//! `rewrite/RULINGS.md`.
//!
//! Contents: the modules below (kernel, circuit owner, run owner, in three groups), and
//! re-exports:
//! - kernel: `Design`, `ItemRow`, `DetailRow`, `Builder`, `Editor`, the handles of `id` (`NameId`, `FileId`,
//!   `ScopeId`, `ExprId`, `DefId`, `TextId`, `NumId`, `ItemRef`, `Ref`, `List` and its aliases),
//!   `Span`, `Table`, the kernel enums of `item` and `expr`, `Literal`, `NumForm`, `RowFacts`,
//!   `Handle`, `Feature`, `FeatureSet`, `Extra`, `features_used`, `link`, `check`,
//!   `check_legal`, `Violation`, the diagnostic rows of `diag`, `eval`, `Bindings`, `Domain`,
//!   `dump`, `parse`
//! - circuit owner: `ElemHead`, `DetailKind`, `ElemKind`, `Element`, `NO_ROW`, `Model`,
//!   `ModelFamily`, `Polarity`, `Wave`, `WaveKind`, `WaveRef`, `WaveRow`
//! - run owner: `Analysis`, `AnalysisKind`, `RunDetail`, `RunDetails`, `Edge`, `Event`,
//!   `EventKind`, `MeasureKind`, `Stat`, `Env`, `IcScope`, `NodeSet`, `NodeSetKind`,
//!   `OptionRow`, `OptionClass`, `OptionKey`, `Measure`, `Print`, `PrintView`, `Save`, `Alter`,
//!   `Loop`, `LoopEnd`, `LoopKind`, `Target`, `TargetKind`, `TargetList`, `AcPart`, `Probe`,
//!   `ProbeKind`, `ProbeList`, `Statistics`, `Vary`, `VaryDist`, `SweepBounds`, `SweepEnd`,
//!   `SweepKind`, `SweepSpec`

#![forbid(unsafe_code)]
#![warn(missing_docs)]
// definition phase: bodies are todo!(); remove when they land
#![allow(unused_variables, dead_code)]

// first: its `todo_facts!` macro is textually scoped to the modules declared after it
#[macro_use]
pub mod facts;

pub mod build;
pub mod check;
pub mod design;
pub mod diag;
pub mod edit;
pub mod eval;
pub mod expr;
pub mod feature;
pub mod id;
pub mod item;
pub mod link;
pub mod names;
pub mod scope;
pub mod span;
pub mod table;
pub mod text;

// circuit owner
pub mod build_circuit;
pub mod controlled;
pub mod element;
pub mod line;
pub mod model;
pub mod passive;
pub mod semi;
pub mod source;
pub mod wave;

pub use build_circuit::ElemHead;
pub use element::{DetailKind, ElemKind, Element, NO_ROW};
pub use model::{Model, ModelFamily, Polarity};
pub use wave::{Wave, WaveKind, WaveRef, WaveRow};

// run owner
pub mod analysis;
pub mod build_run;
pub mod dc_ac;
pub mod measure;
pub mod option;
pub mod option_key;
pub mod output;
pub mod periodic;
pub mod plan;
pub mod probe;
pub mod stats;
pub mod sweep;
pub mod tran;

pub use analysis::{Analysis, AnalysisKind, RunDetail, RunDetails};
pub use build::Builder;
pub use check::{Violation, check, check_legal};
pub use design::{Design, DetailRow, ItemRow};
pub use diag::{Code, Diag, Diags, Info, Kind};
pub use edit::Editor;
pub use eval::{Bindings, Domain, eval};
pub use expr::{BinOp, Dist, DrawPer, DrawWhen, Expr, ExprTag, Func, Literal, NumForm, UnOp};
pub use facts::{Handle, RowFacts};
pub use feature::{Extra, Feature, FeatureSet, features_used};
pub use id::{
    BranchList, DefId, ExprId, ExprList, FileId, ItemRef, List, NameId, NameList, NumId, ParamList,
    Ref, ScopeId, TextId,
};
pub use item::{DefKind, ItemKind, LoadKind, OpaqueClass, Role, ScopeKind};
pub use link::link;
pub use measure::{Edge, Event, EventKind, MeasureKind, Stat};
pub use option::{Env, IcScope, NodeSet, NodeSetKind, OptionRow};
pub use option_key::{OptionClass, OptionKey};
pub use output::{Measure, Print, PrintView, Save};
pub use plan::{Alter, Loop, LoopEnd, LoopKind, Target, TargetKind, TargetList};
pub use probe::{AcPart, Probe, ProbeKind, ProbeList};
pub use span::Span;
pub use stats::{Statistics, Vary, VaryDist};
pub use sweep::{SweepBounds, SweepEnd, SweepKind, SweepSpec};
pub use table::Table;
pub use text::{dump, parse};

// `gen` is a reserved word in edition 2024; this line is last so check_style.py, which does
// not read the raw identifier as a module, sees no import after it.
#[cfg(any(test, feature = "gen"))]
pub mod r#gen;
