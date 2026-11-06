//! One element → its Spectre instance statement (`name (nodes) master
//! k=v`) and the deck's `model` statements (a run of SPICE bins as one
//! model group); the `Spell`/`Refuse` answers for charge capacitors and
//! behavioural sources. A source's waveform and an RF port: `sources.rs`.
//!
//! Cites: Spectre Reference 5.0 `[CMP]` p.628 (`resistor`), p.283
//! (`capacitor`), p.372 (`inductor`), p.577 (`mutual_inductor`),
//! pp.683-686 (`vsource`), p.380 (`isource`), p.680, p.678, p.286, p.288
//! (controlled sources), p.302 (`diode`), p.625 (`relay`), pp.644-645
//! (`tline`); R5 pp.590-616 (`pvcvs` … `pccvs`); `[REF]` 19.1 p.459
//! (bsource `q=`), p.498 (models), p.535 (subcircuit instances); Spectre
//! UG 5.1.41 p.108 (model groups).
//!
//! Contents:
//! - `W::card(&mut self, n: u32) -> Option<String>`, `W::models(&mut self, b: &Block)`

use super::W;
use crate::syntax::behaviour::ModelSemantics;
use crate::syntax::diag::Code;
use crate::syntax::render::capability::Construct;
use crate::syntax::render::read::{Block, Card, Model, Val};

/// `.model` types SPICE defines; a kind outside this list is a Spectre
/// master already.
const SPICE_MODEL_TYPES: &[&str] = &[
    "nmos", "pmos", "npn", "pnp", "d", "njf", "pjf", "nmf", "pmf", "r", "c", "l", "sw", "csw",
    "vdmos", "ltra", "txl", "cpl", "urc",
];

/// A model's Spectre master (`None`: its kind as written) and `type=`.
type Master = (Option<&'static str>, Option<&'static str>);

impl W<'_, '_> {
    /// Element `n`'s statement, with no line end; `None` when it is
    /// refused.
    pub(super) fn card(&mut self, n: u32) -> Option<String> {
        let tok = self.rd.main(n);
        // The declaration's name after its type letter (the name map's).
        let own = &self.names.decl(n)[1..].to_string();
        let line = match self.rd.card_view(n) {
            Card::Rcl {
                letter,
                name: _,
                n1,
                n2,
                value,
                model,
                params,
            } => {
                // A model card read as ngspice maps through `render::models`;
                // another dialect's means other equations.
                let ngspice = self.rd.b.model_semantics == ModelSemantics::Ngspice;
                if model.is_some() && !(ngspice && matches!(letter, 'R' | 'C')) {
                    self.ask(Construct::RclModel, tok);
                    return None;
                }
                let (l, master) = match letter {
                    'R' => ('r', "resistor"),
                    'C' => ('c', "capacitor"),
                    _ => ('l', "inductor"),
                };
                let (name, n1, n2) = (self.dev(own), self.node(&n1), self.node(&n2));
                if ngspice
                    && letter == 'R'
                    && let Some((r, kept)) = self.dependent(&value, model.is_some(), &params, n)?
                {
                    // ngspice's `B` line is noiseless; Spectre's resistor
                    // is, with `isnoisy=no` (`[CMP]` p.629).
                    return Some(format!(
                        "r{name} ({n1} {n2}) resistor r={r}{} isnoisy=no",
                        self.params(&kept)
                    ));
                }
                let params = match ngspice && letter != 'L' {
                    true => self.element_rows(letter, &params, n)?,
                    false => params,
                };
                let master = model.map_or(master.to_string(), |m| self.node(&m));
                let value = match &value {
                    Val::Raw(t) if t.is_empty() => String::new(),
                    v => format!(" {l}={}", self.val(v)),
                };
                format!(
                    "{l}{name} ({n1} {n2}) {master}{value}{}",
                    self.params(&params)
                )
            }
            Card::Charge {
                name: _,
                n1,
                n2,
                charge,
                params,
            } => {
                // Node voltages are lowercase `v(a,b)`; a branch current, a
                // braced parameter or SPICE's `time`, `temper`, `hertz` has
                // no spelling the expression can be trusted to keep.
                let q = charge.replace("V(", "v(");
                let ident = |w: &str| {
                    q.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                        .any(|t| t.eq_ignore_ascii_case(w))
                };
                let bad = ["time", "temper", "hertz"].into_iter().any(ident);
                if q.contains("i(") || q.contains("I(") || q.contains('{') || bad {
                    self.refuse(Code::W0103, tok);
                    return None;
                }
                self.ask(Construct::ChargeCapacitor, tok);
                format!(
                    "c{} ({} {}) bsource q={q}{}",
                    self.dev(own),
                    self.node(&n1),
                    self.node(&n2),
                    self.params(&params)
                )
            }
            // One `mutual_inductor` per pair, the first keeping the name.
            Card::Coupling { ls, k, .. } => crate::syntax::render::spice::pairs(&ls)
                .enumerate()
                .map(|(i, (a, b))| {
                    let (own, k, a, b) = (self.dev(own), self.num(k), self.dev(a), self.dev(b));
                    let n = crate::syntax::render::spice::pair_suffix(i);
                    format!("k{own}{n} mutual_inductor coupling={k} ind1=l{a} ind2=l{b}")
                })
                .collect::<Vec<_>>()
                .join("\n"),
            Card::Source {
                letter,
                name: _,
                np,
                nm,
                value,
                ac_mag,
                ac_phase,
                wave,
                port,
                parasitics: _,
            } => {
                if let Some(port) = port {
                    let ac = (ac_mag, ac_phase);
                    return self.port(tok, own, port, [&np, &nm], (&value, wave.is_some()), ac);
                }
                let (l, master) = match letter {
                    'V' => ('v', "vsource"),
                    _ => ('i', "isource"),
                };
                let wave = match &wave {
                    Some(w) => Some(self.wave(letter, w, tok)?),
                    None => None,
                };
                let mut s = format!(
                    "{l}{} ({} {}) {master} dc={}",
                    self.dev(own),
                    self.node(&np),
                    self.node(&nm),
                    self.val(&value)
                );
                if let Some(mag) = ac_mag {
                    s.push_str(&format!(" mag={}", self.num(mag)));
                    if let Some(phase) = ac_phase {
                        s.push_str(&format!(" phase={}", self.num(phase)));
                    }
                }
                if let Some(w) = wave {
                    s.push(' ');
                    s.push_str(&w);
                }
                s
            }
            Card::Behavioural {
                current,
                np,
                nm,
                expr,
                value,
                ..
            } => self.bsource_card(tok, own, current, [&np, &nm], &expr, value)?,
            Card::Table { .. } => {
                self.ask(Construct::TableSource, tok);
                return None;
            }
            Card::Linear {
                letter,
                name: _,
                nodes,
                gain,
            } => {
                let ns: Vec<String> = nodes.iter().map(|x| self.node(x)).collect();
                let g = self.num(gain);
                let name = self.dev(own);
                match letter {
                    'E' => format!("e{name} ({}) vcvs gain={g}", ns.join(" ")),
                    'G' => format!("g{name} ({}) vccs gm={g}", ns.join(" ")),
                    'F' => format!(
                        "f{name} ({} {}) cccs probe={} gain={g}",
                        ns[0],
                        ns[1],
                        self.dev(&nodes[2])
                    ),
                    _ => format!(
                        "h{name} ({} {}) ccvs probe={} rm={g}",
                        ns[0],
                        ns[1],
                        self.dev(&nodes[2])
                    ),
                }
            }
            Card::Poly {
                letter,
                name: _,
                np,
                nm,
                controls,
                coeffs,
            } => {
                // R5 pp.590-616: the controlling pairs are terminals, the
                // sensing sources `probes`; `coeffs` in SPICE's term order.
                let master = match letter {
                    'E' => "pvcvs",
                    'G' => "pvccs",
                    'F' => "pcccs",
                    _ => "pccvs",
                };
                let l = letter.to_ascii_lowercase();
                let (name, np, nm) = (self.dev(own), self.node(&np), self.node(&nm));
                let coeffs: Vec<String> = coeffs.iter().map(|&c| self.num(c)).collect();
                let coeffs = coeffs.join(" ");
                if matches!(letter, 'E' | 'G') {
                    let cs: Vec<String> = controls.iter().map(|c| self.node(c)).collect();
                    format!(
                        "{l}{name} ({np} {nm} {}) {master} coeffs=[{coeffs}]",
                        cs.join(" ")
                    )
                } else {
                    let cs: Vec<String> = controls.iter().map(|c| self.dev(c)).collect();
                    format!(
                        "{l}{name} ({np} {nm}) {master} probes=[{}] coeffs=[{coeffs}]",
                        cs.join(" ")
                    )
                }
            }
            Card::Semi {
                letter,
                name: _,
                nodes,
                model,
                params,
            } => {
                // `[CMP]` mos models: `d g s b model`; no mapped terminal
                // list for a MOSFET without its bulk or with extra nodes.
                if letter == 'M' && nodes.len() != 4 || letter == 'O' {
                    self.refuse(Code::W0105, tok);
                    return None;
                }
                let params =
                    match letter == 'D' && self.rd.b.model_semantics == ModelSemantics::Ngspice {
                        true => self.element_rows(letter, &params, n)?,
                        false => params,
                    };
                let ns: Vec<String> = nodes.iter().map(|x| self.node(x)).collect();
                format!(
                    "{}{} ({}) {}{}",
                    letter.to_ascii_lowercase(),
                    self.dev(own),
                    ns.join(" "),
                    self.node(&model),
                    self.params(&params)
                )
            }
            Card::Switch {
                letter,
                name: _,
                nodes,
                model,
            } => {
                // `relay` is voltage controlled only (`[CMP]` p.625).
                if letter != 'S' {
                    self.refuse(Code::W0105, tok);
                    return None;
                }
                let ns: Vec<String> = nodes.iter().map(|x| self.node(x)).collect();
                format!(
                    "s{} ({}) {}",
                    self.dev(own),
                    ns.join(" "),
                    self.node(&model)
                )
            }
            Card::Line {
                name: _,
                nodes,
                z0,
                td,
            } => {
                let ns: Vec<String> = nodes.iter().map(|x| self.node(x)).collect();
                format!(
                    "t{} ({}) tline z0={} td={}",
                    self.dev(own),
                    ns.join(" "),
                    self.num(z0),
                    self.num(td)
                )
            }
            Card::Inst {
                name: _,
                ports,
                master,
                params,
            } => {
                let ps: Vec<String> = ports.iter().map(|p| self.node(p)).collect();
                format!(
                    "x{} ({}) {}{}",
                    self.dev(own),
                    ps.join(" "),
                    self.node(&master),
                    self.params(&params)
                )
            }
        };
        Some(line)
    }

    /// Block `b`'s models, one statement each, except that a run of SPICE
    /// bins (`nch.1`, `nch.2`: one base, one master) is one model group:
    /// `model nch bsim4 {` / `1: type=n …` / `}` (UG p.108).
    pub(super) fn models(&mut self, b: &Block) {
        let models: Vec<(Model, u32)> = b
            .models
            .iter()
            .map(|&(m, bin)| (self.rd.model_view(m, bin), self.rd.main(m)))
            .collect();
        let mut i = 0;
        while i < models.len() {
            let (first, tok) = &models[i];
            let Some(master) = self.master(first, *tok) else {
                i += 1;
                continue;
            };
            let run = match bin(first) {
                Some((base, _)) => models[i..]
                    .iter()
                    .take_while(|(m, _)| {
                        bin(m).is_some_and(|(b, _)| b == base) && spectre_master(m) == Some(master)
                    })
                    .count(),
                None => 0,
            };
            let Some((base, _)) = bin(first).filter(|_| run > 0) else {
                let params = match self.table_kind(first) {
                    Some(k) => match self.model_rows(first, b.models[i].0, k) {
                        Some(rows) => self.params(&rows),
                        None => {
                            i += 1;
                            continue;
                        }
                    },
                    None => self.model_params(first, master),
                };
                let line = format!(
                    "model {} {}{params}",
                    self.node(&first.name),
                    master.0.unwrap_or(&first.kind),
                );
                self.line(&line);
                i += 1;
                continue;
            };
            let head = format!(
                "model {} {} {{",
                self.node(base),
                master.0.unwrap_or(&first.kind)
            );
            self.line(&head);
            for (m, _) in &models[i..i + run] {
                let k = bin(m).map_or("", |(_, k)| k);
                let line = format!("{k}:{}", self.model_params(m, master));
                self.line(&line);
            }
            self.line("}");
            i += run;
        }
    }
}

// ---- private ----

impl W<'_, '_> {
    /// A value field: a number in Spectre's suffixes, an expression or raw
    /// text as written.
    fn val(&self, v: &Val) -> String {
        match v {
            Val::Num(x) => self.num(*x),
            Val::Expr(e) | Val::Raw(e) => e.clone(),
        }
    }

    /// Model `m`'s master, or its refusal at `tok`.
    fn master(&mut self, m: &Model, tok: u32) -> Option<Master> {
        let found = match self.table_kind(m) {
            Some(k) => Some((Some(k.spectre), None)),
            None => spectre_master(m),
        };
        if found.is_none() {
            self.refuse(Code::W0104, tok);
        }
        let level = m
            .params
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("level"));
        if found.is_some() && level.is_some_and(|(_, v)| super::spice_num(v.trim()).is_none()) {
            self.note(Code::W0111, tok);
        }
        found
    }

    /// A model's ` type=t k=v …` after its master: a mapped SPICE model
    /// takes no `level` (it chose the master) and lowercase names; a
    /// Spectre master's card is kept.
    fn model_params(&self, m: &Model, (master, ty): Master) -> String {
        let mut s = String::new();
        if let Some(t) = ty {
            s.push_str(&format!(" type={t}"));
        }
        if master.is_none() {
            return s + &self.params(&m.params);
        }
        for (k, v) in m
            .params
            .iter()
            .filter(|(k, _)| !k.eq_ignore_ascii_case("level"))
        {
            let k = k.to_ascii_lowercase();
            match v.is_empty() {
                true => s.push_str(&format!(" {k}")),
                false => s.push_str(&format!(" {k}={}", self.value_text(v))),
            }
        }
        s
    }
}

/// The Spectre master for `m` and its `type=` (`[REF]` p.498); `None` when
/// a SPICE kind (and level) has none: written as its SPICE type it names
/// no Spectre master, and "the nearest model" is other equations.
fn spectre_master(m: &Model) -> Option<Master> {
    let level = m
        .params
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("level"))
        .and_then(|(_, v)| super::spice_num(v.trim()))
        .map_or(1, |(l, _)| l as u32);
    let mos = |l: u32| match l {
        1 => Some("mos1"),
        2 => Some("mos2"),
        3 => Some("mos3"),
        8 | 49 => Some("bsim3v3"),
        14 | 54 => Some("bsim4"),
        _ => None,
    };
    let kind = m.kind.to_ascii_lowercase();
    let found = match kind.as_str() {
        "nmos" => mos(level).map(|m| (m, Some("n"))),
        "pmos" => mos(level).map(|m| (m, Some("p"))),
        "npn" if level == 1 => Some(("bjt", Some("npn"))),
        "pnp" if level == 1 => Some(("bjt", Some("pnp"))),
        "d" if level == 1 => Some(("diode", None)),
        k if !SPICE_MODEL_TYPES.contains(&k) => return Some((None, None)),
        _ => None,
    };
    found.map(|(m, t)| (Some(m), t))
}

/// A SPICE bin's base name and number: `nch.1` is `("nch", "1")`.
fn bin(m: &Model) -> Option<(&str, &str)> {
    m.name
        .rsplit_once('.')
        .filter(|(b, k)| !b.is_empty() && !k.is_empty() && k.bytes().all(|c| c.is_ascii_digit()))
}
