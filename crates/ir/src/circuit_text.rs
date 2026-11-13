//! IR text of the circuit owner's items, and the handle walk over their rows: an `Element` or
//! `Model` row in, its statement line out (`dump_element`, `dump_model`); a row in, every handle
//! it and its detail, side and wave rows hold out (`element_handles`, `model_handles`). The
//! forms below are read back by `circuit_parse.rs`.
//!
//! # Forms
//!
//! ```text
//! element <kind> <name> (<terminal> …) [model=<ref>] [params=(k=v …)] [mult=<value>] <detail>
//! model <name> <family> [polarity=n|p] [bc_sat=areab_only] [sub_sat=emission_ns] [params=(k=v …)] [bin_of=<name>] [ako=<ref>]
//!       [module=<ref>] [version=<value>] [word=<name>]
//! ```
//!
//! `<kind>` and `<family>` are the snake case of the `ElemKind` and `ModelFamily` member
//! (`lossless_line`, `b3_soi_pd`). `<detail>` is the detail row's fields in declaration order,
//! each `key=value` with the field's name as key, a field at its default left out (`Fields`):
//! an expression is a value (`1k`, `$tstep`, `(v(a) * 2)`), a name a name, a list `(a b)` or
//! `[e1, e2]`, an enum its snake-case word, `noisy=false`, a side row `key=(fields)`, a wave
//! `wave=<kind>(fields)` with `<kind>` one of `pulse sin exp pwl sffm am trnoise trrandom
//! pattern`. Example:
//!
//! ```text
//! element capacitor c1 (a 0) value=1p ic=0.5 parasitics=(rser=1m cpar=1) ic_mode=in_op
//! element vsource v1 (in 0) dc=0 wave=pulse(v1=0 v2=1.8 delay=0 rise=$tstep fall=1n width=5u period=10u)
//! model nch bsim4 polarity=n version="4.8"
//! ```
//!
//! `parse` is total: an unknown key or word is a `TEXT_SYNTAX` row and is skipped; the row is
//! pushed through the low tier (`Builder::push`), so a statement that breaks an invariant is
//! built and `check` reports it, as for any producer.
//!
//! Cites: `rewrite/IR-KERNEL.md` §9 (text), §7.2 (row facts, A4 references); `text.rs` grammar;
//! `rewrite/IR-DECISIONS.md` A5.
//!
//! Contents (crate-private): `pub(crate) fn element_handles`, `pub(crate) fn model_handles`,
//! `pub(crate) fn dump_element`, `pub(crate) fn dump_model`, `pub(crate) fn dump_library`, `pub(crate) const KIND_WORDS`,
//! `pub(crate) const FAMILY_WORDS`, `pub(crate) const WAVE_WORDS`, `pub(crate) fn get_row`.
//! The reader of these forms is `circuit_parse.rs`.

use crate::circuit_fields::{Field, Fields, Side};
use crate::controlled::{Behav, Controlled, Delay, FreqTable, Laplace, Poly, TableSource};
use crate::design::{Design, DetailRow};
use crate::element::{DetailKind, ElemKind, Element, MultLaw, NO_ROW};
use crate::facts::Handle;
use crate::id::{ExprId, NameId, Ref};
use crate::item::DefKind;
use crate::library::DeviceLibrary;
use crate::line::Line;
use crate::model::{BcSatArea, Model, ModelFamily, Polarity, SubSatTemp};
use crate::passive::{Coupling, Parasitics, Passive};
use crate::semi::{Semi, Switch};
use crate::source::{Distof, PortSpec, Source};
use crate::text::{self, Ctx};
use crate::wave::{Am, Exp, Pattern, Pulse, Pwl, Sffm, Sin, TrNoise, TrRandom, WaveKind, WaveRef};

/// The IR-text word of every `ElemKind`, in `ALL` order.
pub(crate) const KIND_WORDS: [&str; ElemKind::COUNT] = [
    "resistor",
    "capacitor",
    "inductor",
    "charge_capacitor",
    "flux_inductor",
    "coupling",
    "vsource",
    "isource",
    "port",
    "vcvs",
    "vccs",
    "cccs",
    "ccvs",
    "poly",
    "table",
    "behav",
    "laplace",
    "freq_table",
    "delay",
    "diode",
    "bjt",
    "mosfet",
    "jfet",
    "mesfet",
    "igbt",
    "vswitch",
    "iswitch",
    "lossless_line",
    "lossy_line",
    "rc_line",
    "single_lossy_line",
    "coupled_lines",
    "code_model",
    "module_model",
    "module",
    "instance",
    "unresolved",
];

/// The IR-text word of every `ModelFamily`, in `ALL` order.
pub(crate) const FAMILY_WORDS: [&str; ModelFamily::COUNT] = [
    "resistor",
    "resistor_multiplier",
    "capacitor",
    "capacitor_multiplier",
    "inductor",
    "inductor_multiplier",
    "diode",
    "diode_level3",
    "diode_pspice",
    "diode_pspice_temp",
    "diode_ltspice",
    "diode_ideal",
    "diode_spectre",
    "juncap200",
    "diode_cmc",
    "gummel_poon",
    "vbic",
    "hicum0",
    "hicum2",
    "mextram",
    "hbt_ucsd",
    "mos0",
    "mos1",
    "mos2",
    "mos3",
    "mos6",
    "mos9",
    "bsim1",
    "bsim2",
    "bsim3",
    "bsim4",
    "bsim_soi",
    "b3_soi_fd",
    "b3_soi_dd",
    "b3_soi_pd",
    "b4_soi",
    "soi3",
    "ekv",
    "hisim1",
    "hisim2",
    "hisim_hv",
    "psp103",
    "bsim_bulk",
    "vdmos",
    "vdmos_rpi",
    "jfet_shichman",
    "jfet_parker_skellern",
    "jfet_shockley",
    "jfet_hspice2",
    "mesfet",
    "gaas",
    "mesa",
    "hfet1",
    "hfet2",
    "igbt",
    "switch_hyst",
    "switch_hyst_smooth",
    "switch_log_cubic",
    "current_switch_hyst",
    "current_switch_hyst_smooth",
    "current_switch_log_cubic",
    "relay",
    "ltra",
    "txl",
    "cpl",
    "urc",
    "hspice_w",
    "core_jiles_atherton",
    "source_v",
    "source_i",
    "vcvs",
    "vccs",
    "ccvs",
    "cccs",
    "mutual",
    "tline_ideal",
    "code_model",
    "module",
    "other",
];

/// The IR-text word of every `WaveKind`, in `ALL` order.
pub(crate) const WAVE_WORDS: [&str; WaveKind::COUNT] = [
    "pulse", "sin", "exp", "pwl", "sffm", "am", "trnoise", "trrandom", "pattern",
];

/// Row `index` of `D`'s table, or `None` when out of range (the walks are total over any table
/// contents: a bad index is `check`'s report, never a panic).
pub(crate) fn get_row<D: DetailRow>(design: &Design, index: u32) -> Option<D> {
    D::rows(design).get(index as usize).copied()
}

/// Appends the element's handles: header fields, then its detail row's (side and wave rows
/// recursed into), in field order. A detail, side or wave index out of range contributes
/// nothing.
pub(crate) fn element_handles(e: &Element, design: &Design, out: &mut Vec<Handle>) {
    out.extend([
        Handle::Name(e.name),
        Handle::Names(e.terminals),
        // R5: only a kind's master is a reference (`map_refs` skips any other)
        match e.kind.master_kind() {
            Some(kind) if e.model.name != NameId::NONE => Handle::Ref(e.model, kind),
            _ => Handle::Name(e.model.name),
        },
        Handle::Params(e.params),
        Handle::Expr(e.mult),
    ]);
    match e.kind.detail() {
        DetailKind::None => {}
        DetailKind::Passive => row_handles::<Passive>(design, e.detail, out),
        DetailKind::Coupling => row_handles::<Coupling>(design, e.detail, out),
        DetailKind::Source => row_handles::<Source>(design, e.detail, out),
        DetailKind::Controlled => row_handles::<Controlled>(design, e.detail, out),
        DetailKind::Poly => row_handles::<Poly>(design, e.detail, out),
        DetailKind::Table => row_handles::<TableSource>(design, e.detail, out),
        DetailKind::Behav => row_handles::<Behav>(design, e.detail, out),
        DetailKind::Laplace => row_handles::<Laplace>(design, e.detail, out),
        DetailKind::FreqTable => row_handles::<FreqTable>(design, e.detail, out),
        DetailKind::Delay => row_handles::<Delay>(design, e.detail, out),
        DetailKind::Semi => row_handles::<Semi>(design, e.detail, out),
        DetailKind::Switch => row_handles::<Switch>(design, e.detail, out),
        DetailKind::Line => row_handles::<Line>(design, e.detail, out),
    }
}

/// Appends the model's handles in field order.
pub(crate) fn model_handles(m: &Model, out: &mut Vec<Handle>) {
    out.extend([
        Handle::Name(m.name),
        Handle::Params(m.params),
        Handle::Name(m.bin_of),
        ref_or_name(m.ako, DefKind::Model),
        ref_or_name(m.module, DefKind::Module),
        Handle::Expr(m.version),
        Handle::Name(m.word),
    ]);
}

/// Writes `element <kind> <name> (<terminals>) …` (module header). Asserts the detail index is
/// in range (`dump` is total over designs that pass `check`).
pub(crate) fn dump_element(e: &Element, design: &Design, ctx: &Ctx, out: &mut String) {
    let (lo, hi) = e.kind.terminals();
    let count = e.terminals.len();
    assert!(
        count >= lo as usize && count <= hi as usize,
        "{:?}: terminal count {count}",
        e.kind
    );
    out.push_str(KIND_WORDS[e.kind as usize]);
    out.push(' ');
    text::write_name(design.str(e.name), out);
    out.push(' ');
    text::write_names(design, e.terminals, out);
    if e.model.name != NameId::NONE {
        out.push_str(" model=");
        // the unresolved kind is never linked: any kind writes its name the same way
        let kind = e.kind.master_kind().unwrap_or(DefKind::Subckt);
        text::write_ref(design, e.model, kind, ctx, out);
    }
    if !e.params.is_empty() {
        out.push_str(" params=");
        text::write_params(design, e.params, ctx, out);
    }
    if e.mult != ExprId::NONE {
        out.push_str(" mult=");
        text::write_value(design, e.mult, ctx, out);
    }
    if e.mult_law != MultLaw::Parallel {
        out.push_str(" mult_law=ngspice_append");
    }
    match e.kind.detail() {
        DetailKind::None => assert!(e.detail == NO_ROW, "{:?} has a detail row", e.kind),
        DetailKind::Passive => dump_row::<Passive>(design, e.detail, ctx, out),
        DetailKind::Coupling => dump_row::<Coupling>(design, e.detail, ctx, out),
        DetailKind::Source => dump_row::<Source>(design, e.detail, ctx, out),
        DetailKind::Controlled => dump_row::<Controlled>(design, e.detail, ctx, out),
        DetailKind::Poly => dump_row::<Poly>(design, e.detail, ctx, out),
        DetailKind::Table => dump_row::<TableSource>(design, e.detail, ctx, out),
        DetailKind::Behav => dump_row::<Behav>(design, e.detail, ctx, out),
        DetailKind::Laplace => dump_row::<Laplace>(design, e.detail, ctx, out),
        DetailKind::FreqTable => dump_row::<FreqTable>(design, e.detail, ctx, out),
        DetailKind::Delay => dump_row::<Delay>(design, e.detail, ctx, out),
        DetailKind::Semi => dump_row::<Semi>(design, e.detail, ctx, out),
        DetailKind::Switch => dump_row::<Switch>(design, e.detail, ctx, out),
        DetailKind::Line => dump_row::<Line>(design, e.detail, ctx, out),
    }
}

/// Writes `model <name> <family> …` (module header).
pub(crate) fn dump_model(m: &Model, design: &Design, ctx: &Ctx, out: &mut String) {
    text::write_name(design.str(m.name), out);
    out.push(' ');
    out.push_str(FAMILY_WORDS[m.family as usize]);
    if m.polarity != Polarity::None {
        out.push_str(if m.polarity == Polarity::N {
            " polarity=n"
        } else {
            " polarity=p"
        });
    }
    if m.bc_sat_area == BcSatArea::AreabOnly {
        out.push_str(" bc_sat=areab_only");
    }
    if m.sub_sat_temp == SubSatTemp::EmissionNs {
        out.push_str(" sub_sat=emission_ns");
    }
    if !m.params.is_empty() {
        out.push_str(" params=");
        text::write_params(design, m.params, ctx, out);
    }
    if m.bin_of != NameId::NONE {
        out.push_str(" bin_of=");
        text::write_name(design.str(m.bin_of), out);
    }
    if m.ako.name != NameId::NONE {
        out.push_str(" ako=");
        text::write_ref(design, m.ako, DefKind::Model, ctx, out);
    }
    if m.module.name != NameId::NONE {
        out.push_str(" module=");
        text::write_ref(design, m.module, DefKind::Module, ctx, out);
    }
    if m.version != ExprId::NONE {
        out.push_str(" version=");
        text::write_value(design, m.version, ctx, out);
    }
    if m.word != NameId::NONE {
        out.push_str(" word=");
        text::write_name(design.str(m.word), out);
    }
}

/// Writes a device library's fields: `<family> [path="<path>"]`.
pub(crate) fn dump_library(l: &DeviceLibrary, design: &Design, out: &mut String) {
    out.push_str(FAMILY_WORDS[l.family as usize]);
    if l.path != NameId::NONE {
        out.push_str(" path=");
        text::write_string(design.str(l.path), out);
    }
}

// ---- private ----

/// A reference field as a handle: `Ref` when it names something (`map_refs` visits it), else
/// its `NONE` name.
fn ref_or_name(r: Ref, kind: DefKind) -> Handle {
    if r.name == NameId::NONE {
        Handle::Name(r.name)
    } else {
        Handle::Ref(r, kind)
    }
}

/// The fields of a kind with no detail row: none.
/// Appends the handles of row `index` of `D`'s table, if it exists.
fn row_handles<D: Fields + DetailRow>(design: &Design, index: u32, out: &mut Vec<Handle>) {
    let Some(mut row) = get_row::<D>(design, index) else {
        return;
    };
    row.fields(&mut |_, field| match field {
        Field::Expr(e) => out.push(Handle::Expr(*e)),
        Field::Dyn(e) => out.push(Handle::ExprDyn(*e)),
        Field::Name(n) => out.push(Handle::Name(*n)),
        Field::Names(l) => out.push(Handle::Names(*l)),
        Field::Exprs(l) => out.push(Handle::Exprs(*l)),
        Field::Count(_) | Field::Flag(_) | Field::Choice(_) => {}
        Field::Side(i, Side::Parasitics) => row_handles::<Parasitics>(design, *i, out),
        Field::Side(i, Side::Port) => row_handles::<PortSpec>(design, *i, out),
        Field::Side(i, Side::Distof) => row_handles::<Distof>(design, *i, out),
        Field::Wave(w) => wave_handles(design, *w, out),
    });
}

/// Appends the handles of a waveform row, if it exists.
fn wave_handles(design: &Design, w: WaveRef, out: &mut Vec<Handle>) {
    match w.kind {
        WaveKind::Pulse => row_handles::<Pulse>(design, w.index, out),
        WaveKind::Sin => row_handles::<Sin>(design, w.index, out),
        WaveKind::Exp => row_handles::<Exp>(design, w.index, out),
        WaveKind::Pwl => row_handles::<Pwl>(design, w.index, out),
        WaveKind::Sffm => row_handles::<Sffm>(design, w.index, out),
        WaveKind::Am => row_handles::<Am>(design, w.index, out),
        WaveKind::TrNoise => row_handles::<TrNoise>(design, w.index, out),
        WaveKind::TrRandom => row_handles::<TrRandom>(design, w.index, out),
        WaveKind::Pattern => row_handles::<Pattern>(design, w.index, out),
    }
}

/// Writes ` key=value` for every field of row `index` of `D`'s table not at its default.
fn dump_row<D: Fields + DetailRow>(design: &Design, index: u32, ctx: &Ctx, out: &mut String) {
    let mut row = D::rows(design)[index as usize];
    row.fields(&mut |key, field| dump_field(design, key, field, ctx, out));
}

/// Writes `(fields)` of a side or wave row, with no space after `(`.
fn dump_nested<D: Fields + DetailRow>(design: &Design, index: u32, ctx: &Ctx, out: &mut String) {
    out.push('(');
    let start = out.len();
    dump_row::<D>(design, index, ctx, out);
    if out[start..].starts_with(' ') {
        out.remove(start);
    }
    out.push(')');
}

/// Writes ` key=value` for one field, or nothing at its default.
fn dump_field(design: &Design, key: &str, field: Field<'_>, ctx: &Ctx, out: &mut String) {
    let at_default = match &field {
        Field::Expr(e) | Field::Dyn(e) => **e == ExprId::NONE,
        Field::Name(n) => **n == NameId::NONE,
        Field::Names(l) => l.is_empty(),
        Field::Exprs(l) => l.is_empty(),
        Field::Count(c) => **c == 0,
        Field::Flag(f) => **f,
        Field::Choice(c) => c.get() == 0,
        Field::Side(i, _) => **i == NO_ROW,
        Field::Wave(w) => **w == WaveRef::NONE,
    };
    if at_default {
        return;
    }
    out.push(' ');
    out.push_str(key);
    out.push('=');
    match field {
        Field::Expr(e) | Field::Dyn(e) => text::write_value(design, *e, ctx, out),
        Field::Name(n) => text::write_name(design.str(*n), out),
        Field::Names(l) => text::write_names(design, *l, out),
        Field::Exprs(l) => text::write_exprs(design, *l, ctx, out),
        Field::Count(c) => out.push_str(&c.to_string()),
        Field::Flag(_) => out.push_str("false"),
        Field::Choice(c) => out.push_str(c.words()[c.get()]),
        Field::Side(i, Side::Parasitics) => dump_nested::<Parasitics>(design, *i, ctx, out),
        Field::Side(i, Side::Port) => dump_nested::<PortSpec>(design, *i, ctx, out),
        Field::Side(i, Side::Distof) => dump_nested::<Distof>(design, *i, ctx, out),
        Field::Wave(w) => {
            out.push_str(WAVE_WORDS[w.kind as usize]);
            match w.kind {
                WaveKind::Pulse => dump_nested::<Pulse>(design, w.index, ctx, out),
                WaveKind::Sin => dump_nested::<Sin>(design, w.index, ctx, out),
                WaveKind::Exp => dump_nested::<Exp>(design, w.index, ctx, out),
                WaveKind::Pwl => dump_nested::<Pwl>(design, w.index, ctx, out),
                WaveKind::Sffm => dump_nested::<Sffm>(design, w.index, ctx, out),
                WaveKind::Am => dump_nested::<Am>(design, w.index, ctx, out),
                WaveKind::TrNoise => dump_nested::<TrNoise>(design, w.index, ctx, out),
                WaveKind::TrRandom => dump_nested::<TrRandom>(design, w.index, ctx, out),
                WaveKind::Pattern => dump_nested::<Pattern>(design, w.index, ctx, out),
            }
        }
    }
}
