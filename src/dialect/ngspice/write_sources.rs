//! Independent sources (S3.1b): an `Element` of kind `VSource`, `ISource` or `Port` → its
//! ngspice card, `name n+ n- [dc v] [ac mag [phase]] [distof1 …] [distof2 …] [portnum n z0 z]
//! [wave(values)] [td=v] [r=v] [m=v]`, the inverse of `read_sources` on the rows it produces.
//!
//! A wave's values go in ngspice's positional order (`read_sources/wave.rs`), up to its last
//! given field; a field is what ngspice reads there under `GivenZero::SpiceDefault` (A17), so
//! an omitted one is left off and a given one written as it is. `admits` refuses what the
//! order cannot spell: a gap (an omitted field before a given one), an AM amplitude other than
//! 1, an SFFM of the other card kind's form, a PWL `td`/`r` on a current source, a value
//! numparam cannot evaluate (the time, the circuit, the temperature, a `TStep`/`TStop` leaf).
//!
//! Cites: ngspice-45 `vsrcload.c`, `isrcload.c`, `vsrcpar.c`; `rewrite/AMEND-A12.md` A17;
//! `rewrite/DIALECT-NGSPICE.md` §4.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) const UNSPELLED: ir::Code`,
//! `pub(super) fn admits(&ir::Design, ir::ItemRef, &mut ir::Diags)`,
//! `pub(super) fn source(w: &mut Out<'_>, item: ir::ItemRef)`

use super::codes::Row;
use super::expr_write;
use super::read_sources;
use super::tables::Eval;
use super::write::Out;
use crate::ir;
use crate::ir::wave as wv;

/// What this area's writer spells: exactly what `read_sources` produces (law L5).
pub(super) const FEATURES: ir::FeatureSet = read_sources::FEATURES;

/// A source ngspice's card cannot spell for a reason no feature states.
pub(super) const UNSPELLED: ir::Code = ir::Code::new(b'W', 1150);

/// This area's codes.
pub(super) const CODES: &[Row] = &[(
    UNSPELLED,
    ir::Info {
        title: "a source ngspice's card cannot spell",
        cite: "ngspice-45 vsrcload.c, isrcload.c, vsrcpar.c; rewrite/AMEND-A12.md A17",
        explain: "a wave field omitted before a given one (ngspice reads values by position), an AM amplitude other than 1, an SFFM whose delay or pre-delay value is not its card kind's, a PWL td or r on a current source, an instance parameter, or a value numparam cannot evaluate (the time, the circuit, the temperature, TSTEP, TSTOP).",
        kind: ir::Kind::Refusal,
    },
)];

/// What a wave writes beyond its positional values: its PWL keywords and whether its fixed
/// fields are ngspice's.
struct Extra {
    values: Vec<ir::ExprId>,
    ok: bool,
}

/// Appends `UNSPELLED` at a source ngspice cannot spell (R21).
pub(super) fn admits(design: &ir::Design, item: ir::ItemRef, diags: &mut ir::Diags) {
    let e: ir::Element = design.row(item.index);
    if e.kind.detail() != ir::DetailKind::Source || spells(design, &e) {
        return;
    }
    diags.push(UNSPELLED, design.span_of(item), 0);
}

/// One source card.
pub(super) fn source(w: &mut Out<'_>, item: ir::ItemRef) {
    let d = w.design;
    let e: ir::Element = d.row(item.index);
    assert!(
        e.kind.detail() == ir::DetailKind::Source && spells(d, &e),
        "ngspice write: a source admits passed"
    );
    let s = d.source(&e);
    let mut out = d.str(e.name).to_string();
    for &n in d.names(e.terminals) {
        out.push(' ');
        out.push_str(d.str(n));
    }
    if s.dc_rule == ir::source::DcRule::Given {
        out.push_str(" dc ");
        expr_write::value(d, s.dc, &mut out);
    }
    pair(d, " ac ", s.ac_mag, s.ac_phase_deg, &mut out);
    if s.distof != ir::NO_ROW {
        let t: ir::source::Distof = d.detail(s.distof);
        pair(d, " distof1 ", t.f1_mag, t.f1_phase_deg, &mut out);
        pair(d, " distof2 ", t.f2_mag, t.f2_phase_deg, &mut out);
    }
    if s.port != ir::NO_ROW {
        let p: ir::source::PortSpec = d.detail(s.port);
        out.push_str(" portnum ");
        expr_write::value(d, p.index, &mut out);
        out.push_str(" z0=");
        expr_write::value(d, p.z0, &mut out);
    }
    if !s.wave.is_none() {
        wave(d, s.wave, e.kind == ir::ElemKind::ISource, &mut out);
    }
    if !e.mult.is_none() {
        out.push_str(" m=");
        expr_write::value(d, e.mult, &mut out);
    }
    w.line(&out);
}

// ---- private ----

/// Whether ngspice's card spells the source exactly.
fn spells(d: &ir::Design, e: &ir::Element) -> bool {
    let s = d.source(e);
    let current = e.kind == ir::ElemKind::ISource;
    let mut values = vec![s.dc, s.ac_mag, s.ac_phase_deg, e.mult];
    if s.distof != ir::NO_ROW {
        let t: ir::source::Distof = d.detail(s.distof);
        values.extend([t.f1_mag, t.f1_phase_deg, t.f2_mag, t.f2_phase_deg]);
    }
    if s.port != ir::NO_ROW {
        let p: ir::source::PortSpec = d.detail(s.port);
        values.extend([p.index, p.z0]);
    }
    let mut ok = e.params.is_empty();
    if !s.wave.is_none() {
        let (_, fields, extra) = fields(d, s.wave, current);
        let given = fields
            .iter()
            .rposition(|f| !f.is_none())
            .map_or(0, |k| k + 1);
        ok &= fields[..given].iter().all(|f| !f.is_none()) && extra.ok;
        values.extend(fields[..given].iter().copied());
        values.extend(extra.values);
    }
    ok && values.iter().filter(|v| !v.is_none()).all(|&v| {
        ir::expr_class(d, v) == ir::ExprClass::Constant
            && expr_write::spells_in(d, v, Eval::Numparam)
    })
}

/// A wave's keyword, its positional fields in ngspice's order, and the rest.
fn fields(d: &ir::Design, r: ir::WaveRef, current: bool) -> (&'static str, Vec<ir::ExprId>, Extra) {
    let none = ir::ExprId::NONE;
    let mut extra = Extra {
        values: Vec::new(),
        ok: true,
    };
    let (word, f) = match d.wave(r) {
        ir::Wave::Pulse(p) => (
            "pulse",
            vec![
                p.v1, p.v2, p.delay, p.rise, p.fall, p.width, p.period, p.cycles,
            ],
        ),
        ir::Wave::Sin(s) => (
            "sin",
            vec![
                s.offset,
                s.amplitude,
                s.freq,
                s.delay,
                s.damping,
                s.phase_deg,
            ],
        ),
        ir::Wave::Exp(x) => ("exp", vec![x.v1, x.v2, x.td1, x.tau1, x.td2, x.tau2]),
        ir::Wave::Sffm(s) => {
            let head = [
                s.offset,
                s.amplitude,
                s.signal_freq,
                s.mod_index,
                s.carrier_freq,
            ];
            let mut f = head.to_vec();
            if current {
                // isrcload.c: no delay; the 6th value is the modulation phase
                extra.ok = s.delay.is_none() && s.pre == wv::PreDelay::HoldDelayValue;
                f.extend([s.phase_m_deg, s.phase_c_deg]);
            } else {
                extra.ok = s.pre == wv::PreDelay::Zero;
                f.extend([s.delay, s.phase_m_deg, s.phase_c_deg]);
            }
            ("sffm", f)
        }
        ir::Wave::Am(a) => {
            extra.ok = is_one(d, a.amplitude) && a.mod_time == wv::AmModTime::Delay;
            (
                "am",
                vec![
                    a.offset,
                    a.mod_offset,
                    a.mod_index,
                    a.mod_freq,
                    a.carrier_freq,
                    a.delay,
                    a.phase_m_deg,
                    a.phase_c_deg,
                ],
            )
        }
        ir::Wave::Pwl(p) => {
            extra.ok = !current || p.delay.is_none() && p.repeat_at.is_none();
            extra.values = vec![p.delay, p.repeat_at];
            ("pwl", d.exprs(p.points).to_vec())
        }
        ir::Wave::TrNoise(t) => (
            "trnoise",
            vec![
                t.na,
                t.nt,
                t.nalpha,
                t.namp,
                t.rts_amp,
                t.rts_capture,
                t.rts_emission,
            ],
        ),
        ir::Wave::TrRandom(t) => ("trrandom", vec![t.ts, t.td, t.p1, t.p2]),
        ir::Wave::Pattern(_) => ("", vec![none]),
    };
    (word, f, extra)
}

/// Appends ` word(values)` and, for a PWL, its `td=` and `r=`.
fn wave(d: &ir::Design, r: ir::WaveRef, current: bool, out: &mut String) {
    let (word, f, _) = fields(d, r, current);
    out.push(' ');
    out.push_str(word);
    out.push('(');
    if let ir::Wave::TrRandom(t) = d.wave(r) {
        let k = ir::wave::TrRandomKind::ALL
            .iter()
            .position(|&m| m == t.kind)
            .expect("a member");
        out.push_str(&(k + 1).to_string());
        out.push(' ');
    }
    let given = f.iter().rposition(|x| !x.is_none()).map_or(0, |k| k + 1);
    for (k, &x) in f[..given].iter().enumerate() {
        if k > 0 {
            out.push(' ');
        }
        expr_write::value(d, x, out);
    }
    out.push(')');
    if let ir::Wave::Pwl(p) = d.wave(r) {
        for (key, x) in [(" td=", p.delay), (" r=", p.repeat_at)] {
            if !x.is_none() {
                out.push_str(key);
                expr_write::value(d, x, out);
            }
        }
    }
}

/// Appends `key mag [phase]` when `mag` is given; a phase of exactly `0` is left off (the
/// reader's default).
fn pair(d: &ir::Design, key: &str, mag: ir::ExprId, phase: ir::ExprId, out: &mut String) {
    if mag.is_none() {
        return;
    }
    out.push_str(key);
    expr_write::value(d, mag, out);
    let zero = matches!(d.expr(phase), ir::Expr::Num(n) if d.num(n) == ir::Literal::ZERO);
    if !phase.is_none() && !zero {
        out.push(' ');
        expr_write::value(d, phase, out);
    }
}

/// Whether `e` is a literal of value 1.
fn is_one(d: &ir::Design, e: ir::ExprId) -> bool {
    let one = ir::Literal {
        mantissa: 1,
        frac: 0,
        exp10: 0,
        form: ir::NumForm::Plain,
    };
    matches!(d.expr(e), ir::Expr::Num(n) if d.num(n).eq_value(one))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `admits` refuses the one element of `line` (IR text).
    fn refused(line: &str) -> bool {
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        let text = format!("file \"t.sp\" deck\n  param k = 1\n  {line}\n");
        ir::parse(&text, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "{line}: {diags:?}");
        let item = *design
            .items(design.file(design.entry()).top)
            .last()
            .expect("the element");
        assert_eq!(item.kind, ir::ItemKind::Element);
        admits(&design, item, &mut diags);
        let codes: Vec<ir::Code> = diags.rows().iter().map(|d| d.code).collect();
        assert!(codes.iter().all(|&c| c == UNSPELLED), "{codes:?}");
        !codes.is_empty()
    }

    #[test]
    fn admits_refuses_what_the_positional_card_cannot_spell() {
        // Oracle: ngspice's positional orders (vsrcload.c, isrcload.c) and numparam, which has
        // no time, probe, temperature, TSTEP or TSTOP (runs `{time*1}`, `{temper}`: fatal).
        let wave = |w: &str| format!("element vsource v1 (a 0) wave={w} dc_rule=wave_at_zero");
        let refusals = [
            wave("sin(offset=0 amplitude=1 delay=1u pre=hold_delay_value)"),
            wave("am(offset=0 amplitude=2 mod_offset=1 pre=zero)"),
            wave("sffm(offset=0 amplitude=1 law=ngspice pre=hold_delay_value)"),
            wave("pulse(v1=0 v2=1 delay=0 rise=$tstep)"),
            "element isource i1 (a 0) wave=pwl(points=[0, 0, 1u, 1] delay=1u) dc_rule=wave_at_zero"
                .to_string(),
            "element isource i1 (a 0) wave=sffm(offset=0 amplitude=1 delay=1u law=ngspice \
             pre=hold_delay_value) dc_rule=wave_at_zero"
                .to_string(),
            "element vsource v1 (a 0) dc=($time * 2)".to_string(),
            "element vsource v1 (a 0) dc=$temp".to_string(),
        ];
        for line in &refusals {
            assert!(refused(line), "{line}");
        }
        let spelled = [
            wave("sin(offset=0 amplitude=1 freq=0 delay=1u pre=hold_delay_value)"),
            wave("am(offset=0 amplitude=1 mod_offset=1 pre=zero)"),
            wave("sffm(offset=0 amplitude=1 law=ngspice pre=zero)"),
            wave("pwl(points=[0, 0, 1u, (k * 2)] delay=1u repeat_at=0 repeat=from)"),
            "element isource i1 (a 0) mult=2 wave=sffm(offset=0 amplitude=1 carrier_freq=1k \
             mod_index=1 signal_freq=1k phase_m_deg=3 law=ngspice pre=hold_delay_value) \
             dc_rule=wave_at_zero"
                .to_string(),
            "element vsource v1 (a 0) dc=(k * 2) ac_mag=1 ac_phase_deg=0".to_string(),
        ];
        for line in &spelled {
            assert!(!refused(line), "{line}");
        }
    }
}
