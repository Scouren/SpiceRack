//! `Element` and `Model` rows → Spectre text, the reader's image (`read_elements.rs`,
//! `read_sources.rs`) read back: `name (nodes) master k=v …` with the master a primitive
//! (`resistor`, `capacitor`, `inductor`, `vsource`, `isource`, `vcvs`, `vccs`, `cccs`, `ccvs`,
//! `bsource`), the element's model, or its subcircuit; each typed field by Spectre's key, then
//! the untyped parameters in order. `model name master [type=…] k=v …`.
//!
//! Contents: `element`, `model`, `own_keys` (crate-private)

use super::write::Writer;
use crate::ir;

/// The keys an element of `kind` spells for its own fields: no untyped parameter may use one.
pub(super) fn own_keys(kind: ir::ElemKind) -> &'static [&'static str] {
    use ir::ElemKind as K;
    match kind {
        K::Resistor => &["r", "m", "trise", "tc1", "tc2", "isnoisy"],
        K::Capacitor => &["c", "m", "trise", "tc1", "tc2", "isnoisy"],
        K::Inductor => &["l", "m", "trise", "tc1", "tc2", "isnoisy"],
        K::Diode | K::Bjt => &["area", "m", "trise"],
        K::Mosfet => &["m", "trise"],
        K::Instance => &["m"],
        _ => &[],
    }
}

/// One element.
pub(super) fn element(w: &mut Writer<'_, '_>, row: &ir::Element) {
    use ir::ElemKind as K;
    let d = w.design;
    assert!(
        row.mult_law == ir::MultLaw::Parallel,
        "FEATURES lacks MultInstanceNgspiceAppend"
    );
    let master = match row.kind {
        K::Resistor | K::Capacitor | K::Inductor if row.model.name.is_none() => match row.kind {
            K::Resistor => "resistor",
            K::Capacitor => "capacitor",
            _ => "inductor",
        },
        K::VSource => "vsource",
        K::ISource => "isource",
        K::Vcvs => "vcvs",
        K::Vccs => "vccs",
        K::Cccs => "cccs",
        K::Ccvs => "ccvs",
        K::Behav | K::ChargeCapacitor => "bsource",
        _ => d.str(row.model.name),
    };
    w.line();
    let nodes: Vec<&str> = d.names(row.terminals).iter().map(|&n| d.str(n)).collect();
    w.out.push_str(&format!(
        "{} ({}) {master}",
        d.str(row.name),
        nodes.join(" ")
    ));
    match row.kind {
        K::Resistor | K::Capacitor | K::Inductor => passive(w, row),
        K::VSource | K::ISource => source(w, row),
        K::Vcvs | K::Vccs | K::Cccs | K::Ccvs => {
            let c = d.controlled(row);
            let key = match row.kind {
                K::Vccs => "gm",
                K::Ccvs => "rm",
                _ => "gain",
            };
            if !c.sense.is_none() {
                w.out.push_str(&format!(" probe={}", d.str(c.sense)));
            }
            w.value(key, c.gain);
        }
        K::Behav | K::ChargeCapacitor => {
            let b = d.behav(row);
            let key = match b.output {
                ir::controlled::OutKind::Voltage => "v",
                ir::controlled::OutKind::Current => "i",
                ir::controlled::OutKind::Charge => "q",
                other => unreachable!("FEATURES lacks the {other:?} output"),
            };
            w.value(key, b.expr);
        }
        K::Diode | K::Bjt | K::Mosfet => {
            let s = d.semi(row);
            if !s.area.is_none() {
                w.value("area", s.area);
            }
            if !s.dtemp.is_none() {
                w.value("trise", s.dtemp);
            }
        }
        K::Instance | K::Unresolved => {}
        other => unreachable!("FEATURES lacks {other:?}"),
    }
    if !row.mult.is_none() {
        w.value("m", row.mult);
    }
    for &(k, v) in d.params(row.params) {
        w.value(d.str(k), v);
    }
    w.out.push('\n');
}

/// One model: `model name master [type=…] k=v …`.
pub(super) fn model(w: &mut Writer<'_, '_>, row: ir::Model) {
    use ir::ModelFamily as M;
    let d = w.design;
    let master = match row.family {
        M::Resistor => "resistor",
        M::Capacitor => "capacitor",
        M::Inductor => "inductor",
        M::DiodeSpectre => "diode",
        M::GummelPoon => "bjt",
        M::Mos1 => "mos1",
        M::Mos2 => "mos2",
        M::Mos3 => "mos3",
        other => unreachable!("FEATURES lacks Family({other:?})"),
    };
    w.line();
    w.out
        .push_str(&format!("model {} {master}", d.str(row.name)));
    let bjt = row.family == M::GummelPoon;
    match (row.polarity, bjt) {
        (ir::Polarity::None, _) => {}
        (ir::Polarity::N, true) => w.out.push_str(" type=npn"),
        (ir::Polarity::P, true) => w.out.push_str(" type=pnp"),
        (ir::Polarity::N, false) => w.out.push_str(" type=n"),
        (ir::Polarity::P, false) => w.out.push_str(" type=p"),
    }
    for &(k, v) in d.params(row.params) {
        w.value(d.str(k), v);
    }
    w.out.push('\n');
}

// ---- private ----

/// A resistor's, capacitor's or inductor's typed fields.
fn passive(w: &mut Writer<'_, '_>, row: &ir::Element) {
    let p = w.design.passive(row);
    let key = match row.kind {
        ir::ElemKind::Resistor => "r",
        ir::ElemKind::Capacitor => "c",
        _ => "l",
    };
    for (k, e) in [
        (key, p.value),
        ("trise", p.dtemp),
        ("tc1", p.tc1),
        ("tc2", p.tc2),
    ] {
        if !e.is_none() {
            w.value(k, e);
        }
    }
    if !p.noisy {
        w.out.push_str(" isnoisy=no");
    }
}

/// A source's `dc`, `mag`/`phase` and wave.
fn source(w: &mut Writer<'_, '_>, row: &ir::Element) {
    let d = w.design;
    let s = d.source(row);
    assert!(
        s.dc_rule == ir::source::DcRule::Given,
        "FEATURES lacks the DcAtZero features"
    );
    w.value("dc", s.dc);
    if !s.ac_mag.is_none() {
        w.value("mag", s.ac_mag);
        w.value("phase", s.ac_phase_deg);
    }
    if s.wave.is_none() {
        return;
    }
    match d.wave(s.wave) {
        ir::Wave::Sin(x) => {
            w.out.push_str(" type=sine");
            let fields = [
                ("sinedc", x.offset),
                ("ampl", x.amplitude),
                ("freq", x.freq),
                ("delay", x.delay),
                ("damp", x.damping),
                ("sinephase", x.phase_deg),
            ];
            for (k, e) in fields {
                w.value(k, e);
            }
        }
        ir::Wave::Pulse(x) => {
            w.out.push_str(" type=pulse");
            let fields = [
                ("val0", x.v1),
                ("val1", x.v2),
                ("delay", x.delay),
                ("rise", x.rise),
                ("fall", x.fall),
                ("width", x.width),
                ("period", x.period),
            ];
            for (k, e) in fields {
                w.value(k, e);
            }
        }
        ir::Wave::Pwl(x) => {
            w.out.push_str(" type=pwl wave=[");
            for (k, &p) in d.exprs(x.points).iter().enumerate() {
                if k > 0 {
                    w.out.push(' ');
                }
                super::expr_write::write_item(d, p, w.out);
            }
            w.out.push(']');
            if !x.delay.is_none() {
                w.value("delay", x.delay);
            }
            if !x.repeat_at.is_none() {
                w.value("pwlperiod", x.repeat_at);
                w.value("twidth", x.join_width);
            }
        }
        other => unreachable!("FEATURES lacks {other:?}"),
    }
}
