//! One element card → its SPICE-family line: the element letter from the
//! target's device table, the nodes and values as the reader kept them,
//! numbers in the target's number format, expressions in its delimiters,
//! the waveform, and the `Spell`/`Lift`/`Refuse` answers for charge
//! capacitors, behavioural sources, RF ports, PWL repeats, noise waveforms
//! and Laplace sources.
//!
//! Cites: ngspice manual §3.3.9 (`C1 a b q='…'`), §4.1 (waveforms),
//! §4.1.11 (`portnum`), §5.1-§5.5 (controlled sources), ch.7-ch.11
//! (semiconductors), §2.4.3 (`X`); Xyce RG p.238 (`B … V={…}`); HSPICE
//! `vol=`/`cur=`; LTspice help (`Q=`).
//!
//! Contents:
//! - `W::card(&mut self, n: u32)`, `W::value(&self, v: &Val) -> String`
//! - `pub(super) fn renamed(rd: &Rd, cells: &Cells, card: &Card) -> Option<String>`
//! - `pub(in crate::syntax::render) fn pairs<T>(items: &[T]) -> impl Iterator<Item = (&T, &T)>`,
//!   `pub(in crate::syntax::render) fn pair_suffix(i: usize) -> String`

use super::W;
use super::laplace::{self, Laplace};
use super::parasitics;
use super::ternary::Read;
use super::xyce_cards::xyce_shape;
use crate::syntax::behaviour::{Coupling, NetlistRules, Parasitics};
use crate::syntax::diag::Code;
use crate::syntax::lang::Cells;
use crate::syntax::render::capability::{Action, Alt, Construct, Rule};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::{Card, Rd, Val};

impl W<'_, '_> {
    /// Element `n`'s card, with no line end (a Laplace source is two
    /// lines). With `coupled` (an LTspice deck's top or definition, and the
    /// inductors its `K` cards couple), an uncoupled inductor with no
    /// `Rser` carries LTspice's implicit `Rser=1m`.
    pub(super) fn card(&mut self, n: u32, coupled: Option<&[String]>) {
        // The name the letter policy gives the element (`names`).
        let decl = self.names.decl(n).to_string();
        let card = self.rd.card_view(n);
        let line = match card {
            Card::Rcl { .. } => self.rcl_card(n, card, coupled),
            Card::Charge { .. } => self.charge_card(n, card),
            Card::Coupling { .. } => self.coupling_card(n, card),
            Card::Source { .. } => {
                let (line, then) = self.source_card(n, card);
                return self.o.token(&line, then);
            }
            Card::Table { .. } => self.table_card(n, card),
            Card::Behavioural { .. } => self.behavioural_card(n, card),
            Card::Linear { nodes, gain, .. } => format!(
                "{decl} {} {}",
                nodes.join(" "),
                self.sp.decimal(gain, self.x)
            ),
            Card::Poly { .. } => self.poly_card(n, card),
            Card::Semi { .. } => self.semi_card(n, card),
            Card::Switch { nodes, model, .. } => format!("{decl} {} {model}", nodes.join(" ")),
            Card::Line { nodes, z0, td, .. } => format!(
                "{decl} {} Z0={} TD={}",
                nodes.join(" "),
                self.sp.decimal(z0, self.x),
                self.sp.decimal(td, self.x)
            ),
            Card::Inst { .. } => self.inst_card(n, card),
        };
        self.o.token(&line, Space::None);
    }

    /// A value field: a number in the target's format, an expression in its
    /// delimiters, raw text as written.
    pub(super) fn value(&self, v: &Val) -> String {
        match v {
            Val::Num(x) => self.sp.number(*x, self.x),
            Val::Expr(e) => format!("{}{}{}", self.sp.expr.0, self.ternaries(e), self.sp.expr.1),
            Val::Raw(t) => t.clone(),
        }
    }
}

/// The name a target (its `cells`) writes behavioural source `card` under, where its
/// spelling takes another letter: HSPICE's `E1`/`G1` for `B1` (`vol=`,
/// `cur=`), a Laplace lift's `EB1`/`GB1`, ngspice's XSPICE `AB1`. `None`:
/// the map's `B1`.
pub(super) fn renamed(rd: &Rd, cells: &Cells, card: &Card) -> Option<String> {
    let Card::Behavioural {
        current,
        name,
        value,
        ..
    } = card
    else {
        return None;
    };
    let letter = if *current { 'G' } else { 'E' };
    if !laplace::has_laplace(rd, *value) {
        let vol_cur = cells[Construct::BehaviouralSource as usize] == Action::Spell(Alt::VolCur);
        return vol_cur.then(|| format!("{letter}{name}"));
    }
    // A constant `H(s)` is a plain controlled source on every target.
    match (
        laplace::read(rd, *value),
        cells[Construct::Laplace as usize],
    ) {
        (Some(Laplace::Gain { .. }), _) => Some(format!("{letter}B{name}")),
        (Some(Laplace::Xfer { .. }), Action::Lift(Rule::LaplaceXfer)) => Some(format!("AB{name}")),
        (Some(Laplace::Xfer { .. }), Action::Lift(_)) => Some(format!("{letter}B{name}")),
        // A Laplace source the target refuses is not written.
        (Some(Laplace::Xfer { .. }) | None, _) => None,
    }
}

/// Every pair of `items`, in card order (`a b`, `a c`, `b c`): the couplings
/// of a `K` over more than two inductors.
pub(in crate::syntax::render) fn pairs<T>(items: &[T]) -> impl Iterator<Item = (&T, &T)> {
    items
        .iter()
        .enumerate()
        .flat_map(move |(i, a)| items[i + 1..].iter().map(move |b| (a, b)))
}

/// The name suffix of a `K`'s `i`th pair: none for the first (it keeps the
/// card's name), `_2`, `_3`, … after.
pub(in crate::syntax::render) fn pair_suffix(i: usize) -> String {
    if i == 0 {
        String::new()
    } else {
        format!("_{}", i + 1)
    }
}

// ---- private ----

impl W<'_, '_> {
    /// An R, C or L line: LTspice's implicit `Rser=1m` on an uncoupled L (with
    /// `coupled`), the model where the target reads it, a Xyce R model's
    /// value, `TC=a,b` as ngspice's `tc1`/`tc2`, LTspice parasitics as
    /// elements (`parasitics`).
    fn rcl_card(&mut self, n: u32, card: Card, coupled: Option<&[String]>) -> String {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Rcl {
            letter,
            name,
            n1,
            n2,
            value,
            model,
            mut params,
        } = card
        else {
            unreachable!("an R, C or L card")
        };
        if letter == 'L'
            && let Some(c) = coupled
            && !c.contains(&name)
            && !params.iter().any(|(k, _)| k.eq_ignore_ascii_case("rser"))
        {
            params.push(("Rser".into(), "1m".into()));
        }
        if let Some(m) = &model {
            self.ask(Construct::RclModel, tok);
            self.xyce_instance(n, m, tok);
        }
        self.device_ic(letter, &[&n1, &n2], &params, tok);
        // `TC=a,b` (Xyce, PSpice) is `tc1=a tc2=b` on ngspice
        // (manual §3.3.1: "unknown parameter (0)" for `tc=0,0`).
        if self.b.netlist_rules == NetlistRules::Ngspice
            && let Some(i) = params
                .iter()
                .position(|(k, v)| k.eq_ignore_ascii_case("tc") && v.contains(','))
        {
            let (_, v) = params.remove(i);
            let pairs = ["tc1", "tc2"].iter().zip(v.split(','));
            let pairs: Vec<(String, String)> = pairs
                .map(|(k, c)| (k.to_string(), c.trim().to_string()))
                .collect();
            params.splice(i..i, pairs);
        }
        // An LTspice C's or L's parasitics as elements where the
        // target's C and L take none (`parasitics`).
        let extra = match self.pair.lt_parasitics {
            true if letter != 'R' => parasitics::expand(&decl, &n1, &n2, &mut params),
            _ => None,
        };
        let n1 = extra.as_ref().map_or(n1, |(at, _)| at.clone());
        let mut s = format!("{decl} {n1} {n2}");
        // ngspice-45 reads a C or L model only before the value
        // (`C1 1 0 1u CM`: "unknown parameter (cm)"; run), as
        // HSPICE (`Cxxx n1 n2 [mname] [C=]val`) and Xyce RG do; an R
        // either way.
        let early = model.as_ref().filter(|_| letter != 'R');
        if let Some(m) = early {
            s.push_str(&format!(" {m}"));
        }
        match &value {
            // Xyce's R model `R` multiplies the instance value
            // (default 1k), where ngspice's is the value
            // (`W::r_default`): the instance takes it.
            Val::Raw(t) if t.is_empty() => {
                let given = params.iter().any(|(k, _)| {
                    k.eq_ignore_ascii_case("r") || k.eq_ignore_ascii_case("resistance")
                });
                let model = model.as_deref().filter(|_| !given);
                if let Some(r) = model.and_then(|m| self.r_default(letter, m)) {
                    s.push_str(&format!(" {}", self.spelled(&r)));
                }
            }
            v => s.push_str(&format!(" {}", self.value(v))),
        }
        if let Some(m) = model.as_ref().filter(|_| early.is_none()) {
            s.push_str(&format!(" {m}"));
        }
        s.push_str(&self.params(&params));
        for l in extra.map(|e| e.1).unwrap_or_default() {
            s.push('\n');
            s.push_str(&l);
        }
        s
    }

    /// A charge capacitor (`Q=`), in the target's delimiters.
    fn charge_card(&mut self, n: u32, card: Card) -> String {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Charge {
            n1,
            n2,
            charge,
            params,
            ..
        } = card
        else {
            unreachable!("a charge card")
        };
        let q = match self.ask(Construct::ChargeCapacitor, tok) {
            Action::Spell(Alt::ChargeBraced) => format!("Q={{{charge}}}"),
            Action::Spell(Alt::ChargeBare) => format!("Q={charge}"),
            _ => format!("Q='{charge}'"),
        };
        format!("{decl} {n1} {n2} {q}{}", self.params(&params))
    }

    /// A `K` line: every pair on one card (Xyce, LTspice) or one card per pair.
    fn coupling_card(&mut self, n: u32, card: Card) -> String {
        let decl = self.names.decl(n).to_string();
        let Card::Coupling { ls, refs, k, .. } = card else {
            unreachable!("a coupling card")
        };
        // Each inductor as the map spells its declaration in this
        // scope; one declared elsewhere gains its letter as written.
        let l: Vec<String> = refs
            .iter()
            .zip(&ls)
            .map(|(r, rest)| {
                self.names
                    .reference(self.scope, r, 'L')
                    .map_or_else(|| format!("L{rest}"), str::to_string)
            })
            .collect();
        let k = self.sp.decimal(k, self.x);
        // Xyce and LTspice couple every pair on one card (RG `K`,
        // LTspice help `K`); the others one card per pair, the
        // first keeping the name.
        if self.b.coupling == Coupling::OneCard {
            format!("{decl} {} {k}", l.join(" "))
        } else {
            pairs(&l)
                .enumerate()
                .map(|(i, (a, b))| format!("{decl}{} {a} {b} {k}", pair_suffix(i)))
                .collect::<Vec<_>>()
                .join("\n")
        }
    }

    /// An independent source: its value, AC, port, waveform and parasitics;
    /// Xyce's RF port a `P` line of its own (the `Space` after it: a line
    /// end for that one, none for the others).
    fn source_card(&mut self, n: u32, card: Card) -> (String, Space) {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Source {
            letter,
            np,
            nm,
            value,
            ac_mag,
            ac_phase,
            wave,
            port,
            mut parasitics,
            ..
        } = card
        else {
            unreachable!("a source card")
        };
        // A counted PULSE where the target's PULSE has no count: the PWL
        // of its pulses, refused where they are none (`Wave::pulses`).
        let wave = match wave {
            Some(w) if self.pair.pulse_as_pwl && w.counted() => match w.pulses() {
                Some(pwl) => Some(pwl),
                None => {
                    self.refuse(crate::syntax::diag::Code::W0108, tok);
                    Some(w)
                }
            },
            w => w,
        };
        // A wave the target's own form would run otherwise is written as
        // a behavioural source (`wave_as_b`); the card stays as a 0
        // source, or as what is left of its DC value (`beside_b`).
        let behav = match &wave {
            Some(w) => self.wave_as_b(w, tok),
            None => None,
        };
        let (wave, value) = match (&behav, wave) {
            (Some(_), Some(w)) => self.beside_b(&w, value, tok),
            (_, wave) => (wave, value),
        };
        if let Some(w) = &wave {
            self.wave_ok(letter, w, tok);
        }
        if let Some((num, z0)) = port
            && self.ask(Construct::RfPort, tok) == Action::Spell(Alt::XycePort)
        {
            // Xyce's port carries no source value here.
            if value != Val::Num(0.0) || ac_mag.is_some() || wave.is_some() {
                self.refuse(crate::syntax::diag::Code::W0007, tok);
            }
            let name = decl.strip_prefix('V').filter(|r| r.starts_with(['P', 'p']));
            let name = name.map_or_else(|| format!("P{decl}"), str::to_string);
            let z0 = self.sp.decimal(z0, self.x);
            return (format!("{name} {np} {nm} port={num} z0={z0}"), Space::Line);
        }
        // LTspice writes its own; the others as elements.
        let extra = match self.b.parasitics {
            Parasitics::Ltspice => None,
            Parasitics::Elements | Parasitics::Netlist => {
                parasitics::expand(&decl, &np, &nm, &mut parasitics)
            }
        };
        let np = extra.as_ref().map_or(np, |(at, _)| at.clone());
        // A V is in series with its behavioural source, on node
        // `{decl}_w`; an I in parallel with it.
        let (b, nm) = match behav {
            Some(expr) => {
                let at = match letter {
                    'V' => format!("{decl}_w"),
                    _ => np.clone(),
                };
                let line = self.wave_b(&decl, [&at, &nm], letter == 'I', &expr, tok);
                (Some(line), if letter == 'V' { at } else { nm })
            }
            None => (None, nm),
        };
        let mut s = format!("{decl} {np} {nm} {}", self.value(&value));
        if let Some(mag) = ac_mag {
            s.push_str(&format!(" AC {}", self.sp.decimal(mag, self.x)));
            if let Some(phase) = ac_phase {
                s.push_str(&format!(" {}", self.sp.decimal(phase, self.x)));
            }
        }
        if let Some((num, z0)) = port {
            s.push_str(&format!(
                " portnum {num} z0 {}",
                self.sp.decimal(z0, self.x)
            ));
        }
        if let Some(w) = &wave {
            s.push(' ');
            s.push_str(&self.wave(w));
        }
        s.push_str(&self.params(&parasitics));
        for l in extra.map(|e| e.1).unwrap_or_default().into_iter().chain(b) {
            s.push('\n');
            s.push_str(&l);
        }
        (s, Space::None)
    }

    /// An E/G `TABLE`: ngspice's `pwl()` value, or the native `TABLE`.
    fn table_card(&mut self, n: u32, card: Card) -> String {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Table {
            np,
            nm,
            expr,
            points,
            ..
        } = card
        else {
            unreachable!("a table card")
        };
        let act = self.ask(Construct::TableSource, tok);
        let n = |v: f64| self.sp.number(v, self.x);
        match act {
            // ngspice manual, `pwl(x, x1, y1, …)` in an E/G value
            // (straight on past the ends, so a flat point is added
            // each side; ngspice-45 ran it equal to Xyce's TABLE).
            Action::Spell(Alt::PwlValue) => {
                let (x0, y0) = points[0];
                let (xn, yn) = points[points.len() - 1];
                let mut args = vec![format!("{}, {}", n(x0 - 1.0), n(y0))];
                args.extend(points.iter().map(|&(x, y)| format!("{}, {}", n(x), n(y))));
                args.push(format!("{}, {}", n(xn + 1.0), n(yn)));
                format!(
                    "{decl} {np} {nm} value={{pwl({expr}, {})}}",
                    args.join(", ")
                )
            }
            _ => {
                let pts: Vec<String> = points
                    .iter()
                    .map(|&(x, y)| format!("({},{})", n(x), n(y)))
                    .collect();
                format!("{decl} {np} {nm} TABLE {{{expr}}} = {}", pts.join(" "))
            }
        }
    }

    /// A behavioural source: a Laplace lift (`laplace`), HSPICE's
    /// `vol=`/`cur=`, or `V=`/`I=` braced or bare.
    fn behavioural_card(&mut self, n: u32, card: Card) -> String {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Behavioural {
            current,
            np,
            nm,
            expr,
            value,
            ..
        } = card
        else {
            unreachable!("a behavioural card")
        };
        if laplace::has_laplace(self.rd, value) {
            self.laplace(&decl, &np, &nm, value, current, tok)
        } else {
            let expr = self.names.refs(self.scope, &expr);
            // ngspice's B-source parser reads `?:` by C's grammar.
            let expr = self.ternaries_as(&expr, Read::C);
            // The map names it `G1`/`E1` (`renamed`).
            self.b_line(&decl, [&np, &nm], current, &expr, tok)
        }
    }

    /// A `POLY(n)` controlled source.
    fn poly_card(&mut self, n: u32, card: Card) -> String {
        let decl = self.names.decl(n).to_string();
        let Card::Poly {
            letter,
            np,
            nm,
            controls,
            coeffs,
            ..
        } = card
        else {
            unreachable!("a poly card")
        };
        let dims = if matches!(letter, 'E' | 'G') {
            controls.len() / 2
        } else {
            controls.len()
        };
        let coeffs: Vec<String> = coeffs.iter().map(|&c| self.sp.number(c, self.x)).collect();
        format!(
            "{decl} {np} {nm} POLY({dims}) {} {}",
            controls.join(" "),
            coeffs.join(" ")
        )
    }

    /// A semiconductor, `O` or `N` line: its Xyce shape (`xyce_cards`), a
    /// device `IC=` (`initial`), an OSDI module's own model.
    fn semi_card(&mut self, n: u32, card: Card) -> String {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Semi {
            letter,
            mut nodes,
            model,
            mut params,
            ..
        } = card
        else {
            unreachable!("a semiconductor card")
        };
        // An `O` (LTRA) line, an `N` (OSDI) instance.
        let asked = [('O', Construct::LossyLine), ('N', Construct::OsdiInstance)];
        if let Some(&(_, c)) = asked.iter().find(|(l, _)| *l == letter) {
            self.ask(c, tok);
        }
        let shape = self.xyce_instance(n, &model, tok);
        xyce_shape(shape, &decl, &mut nodes, &mut params);
        // An OSDI module's instance (ngspice manual §13); a module
        // named directly takes a model of its own, its parameters.
        let nodes_ref: Vec<&String> = nodes.iter().collect();
        self.device_ic(letter, &nodes_ref, &params, tok);
        let ps = self.params(&params);
        match self.rd.va_module(n) {
            Some(m) if ps.is_empty() => format!(
                ".model {decl}__va {m}\n{decl} {} {decl}__va",
                nodes.join(" ")
            ),
            Some(m) => format!(
                ".model {decl}__va {m} ({})\n{decl} {} {decl}__va",
                ps.trim_start(),
                nodes.join(" ")
            ),
            None => format!("{decl} {} {model}{ps}", nodes.join(" ")),
        }
    }

    /// An `X` line; an `m` the target ignores where the source multiplies
    /// by it is refused (W0047).
    fn inst_card(&mut self, n: u32, card: Card) -> String {
        let tok = self.rd.main(n);
        let decl = self.names.decl(n).to_string();
        let Card::Inst {
            ports,
            master,
            params,
            ..
        } = card
        else {
            unreachable!("an instance card")
        };
        // ESPice has no subcircuit multiplicity (run by hand: `m=5`
        // on an `X` line changes nothing), where ngspice's `m` is
        // one unless the master declares `m`, Xyce's always.
        let one = |v: &str| v.trim().parse::<f64>() == Ok(1.0);
        let is_m = |(k, v): &(String, String)| k.eq_ignore_ascii_case("m") && !one(v);
        if !self.b.subckt_multiplicity
            && params.iter().any(is_m)
            && match self.rd.b.netlist_rules {
                NetlistRules::Xyce => true,
                NetlistRules::Ngspice => !self.declares(&master, "m"),
                _ => false,
            }
        {
            self.refuse(Code::W0047, tok);
        }
        let mut s = decl;
        for p in &ports {
            s.push(' ');
            s.push_str(p);
        }
        format!("{s} {master}{}", self.params(&params))
    }

    /// ` k=v` each (a flag bare), values [`W::param_value`].
    fn params(&self, params: &[(String, String)]) -> String {
        let mut s = String::new();
        for (k, v) in params {
            if v.is_empty() {
                s.push_str(&format!(" {k}"));
            } else {
                // A Spectre or VACASK expression is bare; a SPICE instance
                // line reads one only in its delimiters (ngspice §2.8.5).
                let bare = self.rd.named() && !v.starts_with('"') && !crate::unit::spice_bare(v);
                let v = match bare {
                    true => format!("{}{}{}", self.sp.expr.0, self.spelled(v), self.sp.expr.1),
                    false => self.param_value(v),
                };
                s.push_str(&format!(" {k}={v}"));
            }
        }
        s
    }
}
