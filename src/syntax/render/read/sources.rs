//! SPICE V and I cards → [`Card::Source`] views with their [`Wave`]s (as
//! the old lowering typed them), each card's fields read by
//! the old SPICE grammar's `spec` rules: `dc`, `ac`, a waveform with or
//! without parentheses, ngspice's named fields.
//!
//! Cites: the old lowering; the old SPICE grammar's `V`, `I`, `spec`,
//! `Sdc`, `Sac`, `wave`, `wargs`; the old ngspice grammar's `Pair_src`, `Trnoise`,
//! `Trrandom`; ngspice manual §4.1.4-§4.1.11.
//!
//! Contents:
//! - `pub(in crate::syntax::render) enum Num`, `struct Wave`, `enum WaveKind`
//! - `Num::num(&self) -> Option<f64>`, `WaveKind::held(self) -> Option<usize>`, `Wave::at_zero(&self) -> Val`
//! - `Rd::source(&self, n: u32, e: Element, name: String) -> Option<Card>`
//! - `Rd::ramp_startup(&mut self, deck: &Deck) -> Option<u32>`

use super::{Card, Comp, Deck, Rd, Val};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::{CardRules, SourceRules};
use crate::syntax::device::Primitive as Element;

/// A waveform value: a number or an expression (without delimiters).
#[derive(Clone, PartialEq, Debug)]
pub(in crate::syntax::render) enum Num {
    /// A number.
    Num(f64),
    /// An expression, its delimiters dropped.
    Expr(String),
}

/// A source waveform: its kind and its values in the card's order, each
/// past the card's last read as the kind's default (`0`; TRRANDOM's `p1`
/// is `1`).
#[derive(Clone, PartialEq, Debug)]
pub(in crate::syntax::render) struct Wave {
    pub(in crate::syntax::render) kind: WaveKind,
    /// The values: SIN 6, PULSE 7, EXP 6, SFFM 5, TRNOISE 7, TRRANDOM 5
    /// (the type first), PWL its pairs flat; AM 8 in ngspice-45's order
    /// `VO VMO VMA FM FC TD PHASEM PHASEC`, the value `VO + (VMO + VMA
    /// sin(2π FM t' + PHASEM)) sin(2π FC t' + PHASEC)`, t' = t - TD, 0
    /// before TD (ngspice-45 `vsrcload.c`).
    pub(in crate::syntax::render) args: Vec<Num>,
    /// PWL `r=`, `td=`.
    pub(in crate::syntax::render) repeat: Option<Num>,
    pub(in crate::syntax::render) delay: Option<Num>,
    /// The statement gave more values than `args` holds (ngspice's PULSE
    /// count NP): the rest are not written.
    pub(in crate::syntax::render) cut: bool,
}

/// A waveform's kind, by its keyword.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::syntax::render) enum WaveKind {
    Pulse,
    Sin,
    Pwl,
    Exp,
    Sffm,
    Am,
    Trnoise,
    Trrandom,
}

/// The length of LTspice's start-up ramp, seconds (LTspice help, `.TRAN`:
/// `startup` ramps the sources on "in first 20u"; LTspice 26.1.1 ran it
/// linear).
const STARTUP_RAMP_S: f64 = 20e-6;

/// One `spec` of a V or I card, the items it holds.
enum Spec {
    /// `dc [v]`.
    Dc(Option<u32>),
    /// `ac [mag [phase]]`.
    Ac(Vec<u32>),
    /// A waveform and its values.
    Wave(WaveKind, Vec<u32>),
    /// ngspice `r=`, `td=`, `portnum n`, `z0 x`: key and value.
    Pair(String, u32),
}

impl<'a> Rd<'a> {
    /// A V or I card (the old SPICE grammar's `V`, `spec`; the old lowering).
    pub(in crate::syntax::render) fn source(
        &self,
        n: u32,
        e: Element,
        name: String,
    ) -> Option<Card> {
        let items = &self.items(n)[..];
        if items.len() < 2 || !self.is_word(items[0]) || !self.is_word(items[1]) {
            return None;
        }
        let (np, nm) = (self.text(items[0]), self.text(items[1]));
        if self.b.card_rules == CardRules::Xyce && self.letter(n) == 'P' {
            return self.xyce_port(&items[2..], name, np, nm);
        }
        let mut j = 2;
        let mut lead = None;
        if let Some(&v) = items.get(2)
            && self.spec(items, 2).is_none()
            && self.tag(v) != Tag::Assign
        {
            if !self.val_ok(v) {
                return None;
            }
            lead = Some(v);
            j = 3;
        }
        let mut specs = Vec::new();
        while j < items.len() {
            let (s, next) = self.spec(items, j)?;
            specs.push(s);
            j = next;
        }
        let (mut value, mut dc_given, mut ac_magnitude, mut ac_phase, mut wf) =
            (Val::Num(0.0), false, None, None, None);
        let (mut repeat, mut delay, mut portnum, mut z0) = (None, None, None, None);
        let mut parasitics = Vec::new();
        if let Some(v) = lead {
            (value, dc_given) = (self.value(v), true);
        }
        for s in specs {
            match s {
                // LTspice help, Voltage Source: `Rser=`, `Cpar=`.
                Spec::Pair(key, val)
                    if self.b.card_rules == CardRules::Ltspice
                        && e == Element::VoltageSource
                        && matches!(key.as_str(), "rser" | "cpar") =>
                {
                    self.irnum(val)?;
                    let key = if key == "rser" { "Rser" } else { "Cpar" };
                    parasitics.push((key.to_string(), self.vtext(val)));
                }
                Spec::Pair(key, val) => {
                    let v = self.irnum(val)?;
                    match key.as_str() {
                        "r" => repeat = Some(v),
                        "td" => delay = Some(v),
                        "portnum" => portnum = Some(self.count(val)?),
                        "z0" => z0 = Some(v.num()?),
                        _ => return None,
                    }
                }
                Spec::Dc(v) => {
                    if let Some(v) = v {
                        (value, dc_given) = (self.value(v), true);
                    }
                }
                Spec::Ac(nums) => {
                    ac_magnitude = Some(nums.first().and_then(|&r| self.number(r)).unwrap_or(1.0));
                    if let Some(&p) = nums.get(1) {
                        ac_phase = Some(self.number(p)?);
                    }
                }
                Spec::Wave(w, vals) => {
                    let vals = self.pwl_inner_keys(w, vals, &mut repeat, &mut delay)?;
                    wf = Some(self.waveform(w, &vals)?)
                }
            }
        }
        if !dc_given && let Some(w) = &wf {
            value = w.at_zero();
        }
        if repeat.is_some() || delay.is_some() {
            let Some(w) = &mut wf else { return None };
            if w.kind != WaveKind::Pwl {
                return None;
            }
            (w.repeat, w.delay) = (repeat, delay);
        }
        if self.ramped.contains(&n) {
            assert!(wf.is_none(), "source: a ramped source is DC only");
            wf = Some(startup_ramp(&value));
        }
        let port = match (portnum, z0) {
            (Some(num), z0) => Some((num, z0.unwrap_or(50.0))),
            (None, None) => None,
            (None, Some(_)) => return None,
        };
        if e != Element::VoltageSource && port.is_some() {
            return None;
        }
        Some(Card::Source {
            letter: if e == Element::VoltageSource {
                'V'
            } else {
                'I'
            },
            name,
            np,
            nm,
            value,
            ac_mag: ac_magnitude,
            ac_phase,
            wave: wf,
            port,
            parasitics,
        })
    }

    /// LTspice's `.tran … startup` for a target with no such start
    /// (`Rule::StartupRamp`): when `deck` runs one, every V and I card at
    /// the top whose value is DC only (with or without `AC`) and not the
    /// number 0 is read from here on with the ramp LTspice 26.1.1 ran,
    /// `PWL(0 0 20u X)` beside its DC value `X`, which `.op`, `.dc` and
    /// `.ac` keep. A waveform source and a subcircuit's source are not
    /// ramped (LTspice help, `.TRAN`). Returns the token of the first
    /// such card whose DC value is kept as raw text: it gets no ramp, so
    /// the writer refuses the deck there.
    ///
    /// # Panics
    /// When called twice on one read, or with a block that is not the top.
    pub(in crate::syntax::render) fn ramp_startup(&mut self, deck: &Deck) -> Option<u32> {
        assert!(self.ramped.is_empty(), "ramp_startup: once a read");
        assert_eq!(deck.top.node, NONE, "ramp_startup: the deck's top");
        self.tran_startup(&deck.bench)?;
        let mut unramped = None;
        for &c in &deck.top.comps {
            let Comp::Card(n) = c else { continue };
            let Card::Source {
                value,
                wave: None,
                port: None,
                ..
            } = self.card_view(n)
            else {
                continue;
            };
            match value {
                Val::Num(v) if v == 0.0 => {}
                Val::Num(_) | Val::Expr(_) => {
                    self.ramped.insert(n);
                }
                Val::Raw(_) => unramped = unramped.or(Some(self.main(n))),
            }
        }
        unramped
    }

    /// Xyce's `P n+ n- port=n [z0=z]` (Xyce RG, port device: z0 50 when
    /// not given) as ngspice's port source `V … portnum n z0 z` (ngspice
    /// manual, `.sp`: Xyce 7.10's `.LIN` and ngspice-45's `.sp` gave the
    /// same S, run); `None` for a port with a source value or a waveform.
    fn xyce_port(&self, rest: &[u32], name: String, np: String, nm: String) -> Option<Card> {
        let (mut num, mut z0) = (None, 50.0);
        for &a in rest {
            if self.tag(a) != Tag::Assign {
                return None;
            }
            let v = self.ast.rhs(Node(a));
            match self.key(a).to_ascii_lowercase().as_str() {
                "port" => num = Some(self.count(v)?),
                "z0" => z0 = self.number(v)?,
                _ => return None,
            }
        }
        Some(Card::Source {
            letter: 'V',
            name: format!("P{name}"),
            np,
            nm,
            value: Val::Num(0.0),
            ac_mag: None,
            ac_phase: None,
            wave: None,
            port: Some((num?, z0)),
            parasitics: vec![],
        })
    }
}

impl Num {
    /// The number, when this is one.
    pub(in crate::syntax::render) fn num(&self) -> Option<f64> {
        match self {
            Num::Num(v) => Some(*v),
            Num::Expr(_) => None,
        }
    }
}

impl WaveKind {
    /// How many values a wave of this kind holds ([`Wave::args`]): one row
    /// per kind; a card's values past it are cut ([`Wave::cut`]). `None`:
    /// every value (PWL's pairs).
    pub(in crate::syntax::render) const fn held(self) -> Option<usize> {
        match self {
            WaveKind::Sin | WaveKind::Exp => Some(6),
            WaveKind::Pulse | WaveKind::Trnoise => Some(7),
            WaveKind::Sffm | WaveKind::Am | WaveKind::Trrandom => Some(5),
            WaveKind::Pwl => None,
        }
    }
}

impl Wave {
    /// The source's value at time zero when the card gives no DC value.
    pub(in crate::syntax::render) fn at_zero(&self) -> Val {
        let v = |n: &Num| match n {
            Num::Num(v) => Val::Num(*v),
            Num::Expr(e) => Val::Expr(e.clone()),
        };
        match self.kind {
            WaveKind::Sin | WaveKind::Sffm | WaveKind::Pulse | WaveKind::Exp => v(&self.args[0]),
            WaveKind::Pwl => self.args.get(1).map_or(Val::Num(0.0), v),
            // TRNOISE and TRRANDOM add to the card's DC value instead of
            // replacing it, so a card with none has DC 0.
            WaveKind::Am | WaveKind::Trnoise | WaveKind::Trrandom => Val::Num(0.0),
        }
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// ngspice-45 also reads PWL's `r=`/`td=` inside the parentheses
    /// (`pwl(0 0 1m 1 r=0 td=0.5m)`: run, v(2.25 ms) = 0.75 V, as `pwl(0
    /// 0 1m 1) r=0 td=0.5m`); their `=` is no item, so `r 0 td 0.5m` follow
    /// the points. Waveform `w`'s values `vals` less those keys, each set
    /// once into `repeat` or `delay`; `None` when one does not read. Which
    /// dialects read the keys there is this function's guard alone.
    fn pwl_inner_keys(
        &self,
        w: WaveKind,
        vals: Vec<u32>,
        repeat: &mut Option<Num>,
        delay: &mut Option<Num>,
    ) -> Option<Vec<u32>> {
        let key = |k: u32, w: &str| self.is_word(k) && self.text(k).eq_ignore_ascii_case(w);
        let at = vals.iter().position(|&k| key(k, "r") || key(k, "td"));
        let i = match at {
            Some(i) if self.b.source_rules == SourceRules::Ngspice && w == WaveKind::Pwl => i,
            _ => return Some(vals),
        };
        let tail = &vals[i..];
        if !tail.len().is_multiple_of(2) {
            return None;
        }
        for kv in tail.chunks(2) {
            let v = self.irnum(kv[1])?;
            match () {
                _ if key(kv[0], "r") && repeat.is_none() => *repeat = Some(v),
                _ if key(kv[0], "td") && delay.is_none() => *delay = Some(v),
                _ => return None,
            }
        }
        assert!(i < vals.len(), "a key inside the parentheses");
        Some(vals[..i].to_vec())
    }

    /// The `spec` starting at item `j`, and the item after it.
    fn spec(&self, items: &[u32], j: usize) -> Option<(Spec, usize)> {
        let it = items[j];
        let next = items.get(j + 1).copied();
        let ngspice = self.b.source_rules == SourceRules::Ngspice;
        let numword = |k: u32| self.number(k).is_some();
        if self.word_is(it, "dc") {
            let v = next.filter(|&v| {
                self.tag(v) != Tag::Assign && self.spec(items, j + 1).is_none() && self.val_ok(v)
            });
            return Some((Spec::Dc(v), j + 1 + usize::from(v.is_some())));
        }
        if self.tag(it) == Tag::Assign && self.key(it).eq_ignore_ascii_case("dc") {
            let v = self.ast.rhs(Node(it));
            return self.val_ok(v).then_some((Spec::Dc(Some(v)), j + 1));
        }
        if self.word_is(it, "ac") {
            let mut nums = Vec::new();
            let mut k = j + 1;
            while nums.len() < 2 && items.get(k).is_some_and(|&x| numword(x)) {
                nums.push(items[k]);
                k += 1;
            }
            return Some((Spec::Ac(nums), k));
        }
        if self.tag(it) == Tag::Assign && self.key(it).eq_ignore_ascii_case("ac") {
            let v = self.ast.rhs(Node(it));
            if v == NONE || !numword(v) {
                return None;
            }
            let mut nums = vec![v];
            if let Some(x) = next.filter(|&x| numword(x)) {
                nums.push(x);
                return Some((Spec::Ac(nums), j + 2));
            }
            return Some((Spec::Ac(nums), j + 1));
        }
        let wave_name = match self.tag(it) {
            Tag::Word => Some(it),
            Tag::Group => Some(self.ast.lhs(Node(it))).filter(|&w| w != NONE),
            _ => None,
        };
        if let Some(w) = wave_name.and_then(|w| self.wave(w)) {
            if self.tag(it) == Tag::Group {
                if !self.group_closed(it) {
                    return None;
                }
                let vals = self.group_items(it).to_vec();
                if !vals.iter().all(|&v| self.is_word(v) || self.expr_ok(v)) {
                    return None;
                }
                return Some((Spec::Wave(w, vals), j + 1));
            }
            let mut vals = Vec::new();
            let mut k = j + 1;
            while let Some(&x) = items.get(k)
                && self.is_word(x)
                && self
                    .text(x)
                    .starts_with(|c: char| c.is_ascii_digit() || "+-.".contains(c))
            {
                vals.push(x);
                k += 1;
            }
            return Some((Spec::Wave(w, vals), k));
        }
        // LTspice help, Voltage Source: `Rser=`, `Cpar=`.
        if self.b.card_rules == CardRules::Ltspice
            && self.tag(it) == Tag::Assign
            && ["rser", "cpar"]
                .iter()
                .any(|w| self.key(it).eq_ignore_ascii_case(w))
        {
            let v = self.ast.rhs(Node(it));
            let key = self.key(it).to_ascii_lowercase();
            return self.val_ok(v).then_some((Spec::Pair(key, v), j + 1));
        }
        if ngspice {
            let keys = ["portnum", "z0", "td", "r"];
            if self.tag(it) == Tag::Assign {
                let key = self.key(it).to_ascii_lowercase();
                let v = self.ast.rhs(Node(it));
                if keys.contains(&key.as_str())
                    && self.is_word(self.ast.lhs(Node(it)))
                    && self.val_ok(v)
                {
                    return Some((Spec::Pair(key, v), j + 1));
                }
            }
            if let Some(k) = keys.iter().find(|k| self.word_is(it, k))
                && let Some(v) = next.filter(|&v| self.tag(v) != Tag::Assign && self.val_ok(v))
            {
                return Some((Spec::Pair(k.to_string(), v), j + 2));
            }
        }
        None
    }

    /// An AM's values in ngspice-45's order ([`Wave::args`]). ngspice:
    /// `AM(VO VMO VMA FM FC [TD [PHASEM [PHASEC]]])` (`vsrcload.c`; FM
    /// and FC default to a share of the run's stop time, so they must be
    /// given here); HSPICE: `AM(SA OC FM FC [TD])`, `SA (OC + sin(2π FM
    /// t')) sin(2π FC t')` (HSPICE UG, AM source), so VO 0, VMO SA·OC,
    /// VMA SA. No other SPICE dialect has AM.
    fn am(&self, a: Vec<Num>) -> Option<Wave> {
        let z = || Num::Num(0.0);
        let args = match (self.b.source_rules, self.b.card_rules) {
            (SourceRules::Ngspice, _) if (5..=8).contains(&a.len()) => {
                let mut v = a;
                v.resize(8, z());
                v
            }
            (_, CardRules::Hspice) if (4..=5).contains(&a.len()) => {
                let (sa, oc) = (a[0].num()?, a[1].num()?);
                let td = a.get(4).cloned().unwrap_or_else(z);
                vec![
                    z(),
                    Num::Num(sa * oc),
                    a[0].clone(),
                    a[2].clone(),
                    a[3].clone(),
                    td,
                    z(),
                    z(),
                ]
            }
            _ => return None,
        };
        Some(Wave {
            kind: WaveKind::Am,
            args,
            repeat: None,
            delay: None,
            cut: false,
        })
    }

    /// The waveform a word names.
    fn wave(&self, w: u32) -> Option<WaveKind> {
        let ngspice = self.b.source_rules == SourceRules::Ngspice;
        let t = self.word_text(w)?.to_ascii_lowercase();
        Some(match t.as_str() {
            "pulse" => WaveKind::Pulse,
            "sin" | "sine" => WaveKind::Sin,
            "pwl" => WaveKind::Pwl,
            "exp" => WaveKind::Exp,
            "sffm" => WaveKind::Sffm,
            "am" => WaveKind::Am,
            "trnoise" if ngspice => WaveKind::Trnoise,
            "trrandom" if ngspice => WaveKind::Trrandom,
            _ => return None,
        })
    }

    /// A waveform's values → its view (the old lowering's `waveform`).
    fn waveform(&self, w: WaveKind, vals: &[u32]) -> Option<Wave> {
        let a: Vec<Num> = vals.iter().map(|&v| self.irnum(v)).collect::<Option<_>>()?;
        if w == WaveKind::Am {
            return self.am(a);
        }
        // SFFM's values past its five (ngspice's TD and phases) read only
        // as zeros.
        let sffm_held = WaveKind::Sffm.held().expect("SFFM holds a count");
        if matches!(w, WaveKind::Sffm | WaveKind::Am)
            && a[a.len().min(sffm_held)..]
                .iter()
                .any(|x| *x != Num::Num(0.0))
        {
            return None;
        }
        let padded = |k: usize| -> Vec<Num> {
            (0..k)
                .map(|i| a.get(i).cloned().unwrap_or(Num::Num(0.0)))
                .collect()
        };
        let held = w.held().unwrap_or(a.len());
        let args = match w {
            WaveKind::Pwl if a.len().is_multiple_of(2) => a.clone(),
            WaveKind::Pwl => return None,
            WaveKind::Trrandom => {
                let kind = a
                    .first()?
                    .num()
                    .filter(|t| t.fract() == 0.0 && (1.0..=4.0).contains(t))?;
                let mut args = padded(held);
                args[0] = Num::Num(kind);
                args[1] = a.get(1)?.clone();
                args[3] = a.get(3).cloned().unwrap_or(Num::Num(1.0));
                args
            }
            WaveKind::Sin
            | WaveKind::Exp
            | WaveKind::Pulse
            | WaveKind::Trnoise
            | WaveKind::Sffm
            | WaveKind::Am => padded(held),
        };
        assert_eq!(args.len(), held, "a kind holds its own count");
        Some(Wave {
            kind: w,
            args,
            repeat: None,
            delay: None,
            cut: a.len() > held,
        })
    }
}

/// LTspice's start-up ramp of a source whose DC value is `full`:
/// `PWL(0 0 20u full)`.
///
/// # Panics
/// For a value kept as raw text ([`Rd::ramp_startup`] ramps none).
fn startup_ramp(full: &Val) -> Wave {
    let full = match full {
        Val::Num(v) => Num::Num(*v),
        Val::Expr(e) => Num::Expr(e.clone()),
        Val::Raw(_) => unreachable!("ramp_startup ramps a number or an expression"),
    };
    let zero = || Num::Num(0.0);
    Wave {
        kind: WaveKind::Pwl,
        args: vec![zero(), zero(), Num::Num(STARTUP_RAMP_S), full],
        repeat: None,
        delay: None,
        cut: false,
    }
}
