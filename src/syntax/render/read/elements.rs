//! SPICE element `Instance` nodes → their place in the block (a typed
//! card, an `X` card, or a raw line) and the [`Card`] view the writer
//! spells, each card first checked against the shape its letter's rule in
//! the old SPICE grammar reads (a card that fails it was `Raw_element`,
//! kept raw; one with no field after its name was `Unparsed`). In a kit, an
//! R/C/L card's model word is the one `syntax::resolve` finds among its
//! values. `Rd::component` is the per-letter dispatch (one arm per
//! [`Element`], none left to `_`): R, C, L, K, S, W and T read here, E, F, G
//! and H in `controlled`, POLY, `B`, semiconductors and `X` in `devices`, V
//! and I in `sources`. A new letter's reading is a new arm and its function.
//!
//! Cites: the old lowering's element rules; the old SPICE grammar's §3.1 element rules (`R` … `X`,
//! `inst`, `nodes`, `spec`, `wargs`, `Poly`, `behave`, `B`).
//!
//! Contents:
//! - `pub(in crate::syntax::render) enum Card`
//! - `Rd::element(&mut self, n: u32, here: &mut Block)`
//! - `Rd::card_view(&self, n: u32) -> Card`, `Rd::letter(&self, n: u32) -> char`,
//!   `Rd::node_text(&self, items: &[u32], i: usize) -> Option<String>`

use super::{Block, Comp, Rd, Val, Wave};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::CardRules;
use crate::syntax::device::Primitive as Element;

/// An element card as the writer spells it: what the old reader kept of
/// it, its words as written. `name` is the name after the element letter.
#[derive(Clone, Debug)]
pub(in crate::syntax::render) enum Card {
    /// `R`, `C`, `L`: the value (an empty `Raw` when the word was a model),
    /// the model, the parameters.
    Rcl {
        letter: char,
        name: String,
        n1: String,
        n2: String,
        value: Val,
        model: Option<String>,
        params: Vec<(String, String)>,
    },
    /// `C1 a b q=expr`: the charge's text, and its tree (the value node).
    Charge {
        name: String,
        n1: String,
        n2: String,
        charge: String,
        value: u32,
        params: Vec<(String, String)>,
    },
    /// Xyce's `E`/`G` `TABLE {expr} = (x,y) …`: the output (a current for
    /// G) by straight lines between the points, constant past either end.
    Table {
        current: bool,
        name: String,
        np: String,
        nm: String,
        expr: String,
        points: Vec<(f64, f64)>,
    },
    /// `K1 L1 L2 … k`: the inductor names after their letter, and as the
    /// card wrote them (the references the name map looks up); every pair
    /// coupled by `k` (Xyce RG and LTspice help, `K` with more than two
    /// inductors).
    Coupling {
        name: String,
        ls: Vec<String>,
        refs: Vec<String>,
        k: f64,
    },
    /// `V`, `I`.
    Source {
        letter: char,
        name: String,
        np: String,
        nm: String,
        value: Val,
        ac_mag: Option<f64>,
        ac_phase: Option<f64>,
        wave: Option<Wave>,
        /// ngspice `portnum n z0 Z`.
        port: Option<(u32, f64)>,
        /// LTspice's `Rser=` and `Cpar=` (the source's series resistance,
        /// its parallel capacitance), each key and value text as written.
        parasitics: Vec<(String, String)>,
    },
    /// `B … V=` / `I=`, `E … value=`, `G … cur=`: the expression's text,
    /// and its tree (the value node).
    Behavioural {
        current: bool,
        name: String,
        np: String,
        nm: String,
        expr: String,
        value: u32,
    },
    /// `E`/`G` (two control nodes), `F`/`H` (a controlling source).
    Linear {
        letter: char,
        name: String,
        nodes: Vec<String>,
        gain: f64,
    },
    /// `E o 0 POLY(n) controls… coeffs…`.
    Poly {
        letter: char,
        name: String,
        np: String,
        nm: String,
        controls: Vec<String>,
        coeffs: Vec<f64>,
    },
    /// `D`, `Q`, `M`, `J`, `Z`: nodes, model, parameters.
    Semi {
        letter: char,
        name: String,
        nodes: Vec<String>,
        model: String,
        params: Vec<(String, String)>,
    },
    /// `S` (four nodes), `W` (two nodes and a controlling source): nodes
    /// then the model.
    Switch {
        letter: char,
        name: String,
        nodes: Vec<String>,
        model: String,
    },
    /// `T a b c d Z0= TD=`.
    Line {
        name: String,
        nodes: Vec<String>,
        z0: f64,
        td: f64,
    },
    /// `X`: ports, master (after the name map), parameters.
    Inst {
        name: String,
        ports: Vec<String>,
        master: String,
        params: Vec<(String, String)>,
    },
}

impl<'a> Rd<'a> {
    /// An element card onto `here`: a typed card, a subcircuit instance, or
    /// a raw line.
    pub(super) fn element(&mut self, n: u32, here: &mut Block) {
        assert_eq!(self.tag(n), Tag::Instance, "an element card");
        let before = here.comps.len() + here.insts.len() + here.raw.len();
        self.place_element(n, here);
        let after = here.comps.len() + here.insts.len() + here.raw.len();
        assert_eq!(after, before + 1, "an element placed once");
    }

    /// The view of an element [`Rd::element`] placed as a typed card or an
    /// instance.
    ///
    /// # Panics
    /// When `n` is no such element.
    pub(in crate::syntax::render) fn card_view(&self, n: u32) -> Card {
        self.try_card(n).expect("an element the reader typed")
    }

    /// The card's element letter, uppercased (as the old IR kept it).
    pub(super) fn letter(&self, n: u32) -> char {
        let first = self.t.text(self.main(n)).chars().next();
        first.expect("an element name").to_ascii_uppercase()
    }

    /// Item `i` of an element's `items` as a node name, when it is a word.
    pub(super) fn node_text(&self, items: &[u32], i: usize) -> Option<String> {
        items
            .get(i)
            .copied()
            .filter(|&k| self.is_word(k))
            .map(|k| self.text(k))
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// Element `n` onto `here`; the one place a typed element's [`Card`]
    /// is first built (`try_card`), where a table of views would be filled.
    fn place_element(&mut self, n: u32, here: &mut Block) {
        if !self
            .t
            .text(self.main(n))
            .starts_with(|c: char| c.is_ascii_alphabetic())
        {
            // quirk: the old SPICE grammar's `element_any <- &[a-z] Name`:
            // an element's name starts with a letter; any other line is
            // `Unparsed`.
            return self.unparsed(n, here);
        }
        let has_fields = self.fields_ok(n);
        match self.try_card(n) {
            Some(Card::Inst { .. }) => here.insts.push((here.comps.len() as u32, n)),
            Some(_) => here.comps.push(Comp::Card(n)),
            None if has_fields => self.raw(n, here, false),
            None => self.unparsed(n, here),
        }
    }

    /// What the old reader typed element `n` as; `None` for a raw one.
    fn try_card(&self, n: u32) -> Option<Card> {
        if self.named() {
            return self.named_inst(n);
        }
        let h = self.ast.header(Node(n));
        match Element::from_u32(h.code) {
            None => None,
            Some(Element::Subckt) if h.master == NONE => None,
            Some(Element::Subckt) => self.instance(n),
            Some(e) => self.component(n, e),
        }
    }

    fn component(&self, n: u32, e: Element) -> Option<Card> {
        let items = &self.items(n)[..];
        let name = self.elem_name(n);
        // quirk: every element rule reads `s` between its fields; only an
        // `inst` tail and a waveform's values take commas.
        let commas = matches!(
            e,
            Element::Resistor
                | Element::Capacitor
                | Element::Inductor
                | Element::Diode
                | Element::Bjt
                | Element::Mosfet
                | Element::Jfet
                | Element::Mesfet
                | Element::VoltageSource
                | Element::CurrentSource
        );
        if matches!(e, Element::Vcvs | Element::Vccs)
            && let [a, b, ..] = items
            && self.is_word(*a)
            && self.is_word(*b)
            && let Some(t) = self.table_source(
                n,
                e == Element::Vccs,
                name.clone(),
                self.text(*a),
                self.text(*b),
            )
        {
            return Some(t);
        }
        let head = items.len().min(2);
        // A `B` card's body after `key =` is text to the card's end.
        let spaced = match e {
            Element::Behavioural => items.len().min(3),
            _ => items.len(),
        };
        if !self.spaced_items(self.name_last(n), &items[..head], false)
            || !self.spaced_items(
                items
                    .get(head.wrapping_sub(1))
                    .map_or(self.name_last(n), |&k| self.last(k)),
                &items[head..spaced],
                commas,
            )
        {
            return None;
        }
        let node = |i: usize| self.node_text(items, i);
        match e {
            Element::Resistor | Element::Capacitor | Element::Inductor => {
                self.rcl(n, e, name, node(0)?, node(1)?, &items[2..])
            }
            Element::Coupling => self.coupling(name, items),
            Element::VoltageSource | Element::CurrentSource => self.source(n, e, name),
            Element::Vcvs | Element::Vccs | Element::Cccs | Element::Ccvs => {
                self.controlled(n, e, name, items)
            }
            Element::Behavioural => self.behavioural(n, name),
            Element::Diode | Element::Bjt | Element::Mosfet | Element::Jfet | Element::Mesfet => {
                self.semiconductor(n, e, name)
            }
            // Xyce's and LTspice's `O` (four nodes, an LTRA model) is
            // ngspice's (manual §6.3, Xyce RG lossy transmission line).
            Element::LossyLine
                if matches!(self.b.card_rules, CardRules::Xyce | CardRules::Ltspice) =>
            {
                self.semiconductor(n, e, name)
            }
            Element::VoltageSwitch | Element::CurrentSwitch => self.switch(n, e, name, items),
            Element::TransmissionLine => self.tline(name, items),
            // else: the ngspice letters the old reader had no rule for; a
            // new element's reading fills its own arm here.
            Element::Subckt
            | Element::LossyLine
            | Element::RcLine
            | Element::Txl
            | Element::CoupledLines
            | Element::CodeModel
            | Element::Osdi => None,
        }
    }

    /// `K1 L1 L2 … k` (`items` after the name): ngspice couples two
    /// inductors (manual, coupled inductors), Xyce and LTspice any number.
    fn coupling(&self, name: String, items: &[u32]) -> Option<Card> {
        let (k, ls) = items.split_last()?;
        let many = matches!(self.b.card_rules, CardRules::Xyce | CardRules::Ltspice);
        if ls.len() < 2 || (ls.len() > 2 && !many) {
            return None;
        }
        if !ls.iter().chain([k]).all(|&x| self.is_word(x)) {
            return None;
        }
        let refs: Vec<String> = ls.iter().map(|&x| self.text(x)).collect();
        // Past two, each an inductor's name, once (Xyce also reads
        // `K1 L1 L2 0.75 K2 L1 L3 0.8`, two cards on a line).
        let named = |r: &String| r.starts_with(['l', 'L']);
        if refs.len() > 2
            && (!refs.iter().all(named)
                || refs
                    .iter()
                    .enumerate()
                    .any(|(i, r)| refs[..i].iter().any(|q| q.eq_ignore_ascii_case(r))))
        {
            return None;
        }
        let ls: Vec<String> = refs
            .iter()
            .map(|r| r.get(1..).map(str::to_string))
            .collect::<Option<_>>()?;
        assert_eq!(ls.len(), refs.len(), "one inductor per reference");
        assert!(ls.len() >= 2, "a coupling couples two or more");
        Some(Card::Coupling {
            name,
            ls,
            refs,
            k: self.number(*k)?,
        })
    }

    /// `S` (four nodes and a model) or `W` (two nodes, a controlling
    /// source and a model): `items` after the name.
    fn switch(&self, n: u32, e: Element, name: String, items: &[u32]) -> Option<Card> {
        assert!(matches!(e, Element::VoltageSwitch | Element::CurrentSwitch));
        let want = if e == Element::VoltageSwitch { 5 } else { 4 };
        if items.len() != want || !items.iter().all(|&k| self.is_word(k)) {
            return None;
        }
        let (nodes, m) = items.split_at(want - 1);
        assert_eq!(m.len(), 1, "the model last");
        Some(Card::Switch {
            letter: self.letter(n),
            name,
            nodes: nodes.iter().map(|&k| self.text(k)).collect(),
            model: self.text(m[0]),
        })
    }

    /// `T a b c d Z0= TD=` or `F= [NL=]` (`items` after the name).
    fn tline(&self, name: String, items: &[u32]) -> Option<Card> {
        if items.len() < 4 || !items[..4].iter().all(|&k| self.is_word(k)) {
            return None;
        }
        let pairs = &items[4..];
        if !pairs.iter().all(|&p| self.pair_ok(p)) {
            return None;
        }
        let get = |key: &str| {
            pairs
                .iter()
                .find(|&&p| self.key(p).eq_ignore_ascii_case(key))
                .and_then(|&p| self.number(self.ast.rhs(Node(p))))
        };
        let known = ["z0", "td", "f", "nl"];
        if !pairs
            .iter()
            .all(|&p| known.iter().any(|k| self.key(p).eq_ignore_ascii_case(k)))
        {
            return None;
        }
        // The delay, or a frequency at which the line is `nl`
        // wavelengths long (default a quarter): td = nl / f
        // (ngspice manual and Xyce RG, lossless transmission line `T`).
        let td = match (get("td"), get("f")) {
            (Some(td), None) if pairs.len() == 2 => td,
            (None, Some(f)) if f > 0.0 && pairs.len() <= 3 => get("nl").unwrap_or(0.25) / f,
            _ => return None,
        };
        Some(Card::Line {
            name,
            nodes: items[..4].iter().map(|&k| self.text(k)).collect(),
            z0: get("z0")?,
            td,
        })
    }

    /// Whether model word `m` names a deck `.model` whose every parameter
    /// means the same in ngspice: `TC1`, `TC2` (quadratic temperature
    /// coefficients), `TNOM`. Xyce's `R`, `C`, `L` model parameters of the
    /// same name are multipliers (Xyce RG, resistor, capacitor and
    /// inductor model parameters), ngspice's the default value (manual,
    /// semiconductor resistor and capacitor; Xyce 7.10 runs `L1 2 0 IND 5m`
    /// with `L=2` at 10 mH, ngspice-45 at 5 mH), so such a card stays text.
    fn same_rcl_model(&self, m: u32) -> bool {
        const SAME: [&str; 3] = ["tc1", "tc2", "tnom"];
        let Some(&card) = self.models.get(&self.text(m).to_ascii_lowercase()) else {
            return false;
        };
        self.model_view(card, NONE)
            .params
            .iter()
            .all(|(k, _)| SAME.iter().any(|s| k.eq_ignore_ascii_case(s)))
    }

    /// An R, C or L card's value, model and parameters.
    fn rcl(
        &self,
        n: u32,
        e: Element,
        name: String,
        n1: String,
        n2: String,
        rest: &[u32],
    ) -> Option<Card> {
        let (val, inst) = match rest.first() {
            Some(&v) if self.tag(v) != Tag::Assign => (Some(v), &rest[1..]),
            _ => (None, rest),
        };
        // Xyce's `R1 a b RMOD 1k`: the model before the value (Xyce RG,
        // resistor and capacitor `[model name] [value]`), the value before
        // the model for every other SPICE (ngspice manual, semiconductor
        // resistor and capacitor).
        let mut swapped = None;
        let (val, inst) = match (val, inst.split_first()) {
            (Some(m), Some((&v, tail)))
                if self.b.card_rules == CardRules::Xyce
                    && self.tag(v) != Tag::Assign
                    && self.number(v).is_some()
                    && self.same_rcl_model(m)
                    && tail.iter().all(|&a| {
                        // ngspice's own instance words of the same meaning
                        // (Xyce RG and ngspice manual, R, C, L instance
                        // parameters); Xyce's `TC=a,b` is not ngspice's.
                        self.tag(a) == Tag::Assign
                            && ["temp", "ic", "m", "tc1", "tc2"]
                                .iter()
                                .any(|w| self.key(a).eq_ignore_ascii_case(w))
                    }) =>
            {
                swapped = Some(m);
                (Some(v), tail)
            }
            _ => (val, inst),
        };
        // The kit's model word: the value, or a word after it.
        let kit_model = self.kit_split(n).map(|s| s.master.0);
        let late = swapped.or(kit_model.filter(|m| inst.contains(m)));
        let inst: Vec<u32> = inst.iter().copied().filter(|&k| Some(k) != late).collect();
        let inst = &inst[..];
        if val.is_some_and(|v| !self.val_ok(v) || self.tag(v) == Tag::Seq) || !self.inst_ok(inst) {
            return None;
        }
        // A flag before the parameters (the old lowering); an LTspice
        // capacitor's `x<n>` (n units in parallel) stands anywhere.
        let lt_cap = self.b.card_rules == CardRules::Ltspice && e == Element::Capacitor;
        let units = |x: u32| lt_cap && super::parallel::units(&self.text(x)).is_some();
        let first_pair = inst.iter().position(|&x| self.tag(x) == Tag::Assign);
        if inst
            .iter()
            .position(|&x| self.is_word(x) && !units(x))
            .is_some_and(|f| first_pair.is_none_or(|p| f < p))
        {
            return None;
        }
        let mut params = self.params_of(inst, false);
        let pairs: Vec<u32> = inst
            .iter()
            .copied()
            .filter(|&x| self.tag(x) == Tag::Assign)
            .collect();
        let mut model = None;
        let value = match val {
            Some(v) if late.is_some() => {
                model = late.map(|m| self.text(m));
                self.value(v)
            }
            Some(v)
                if self.is_word(v)
                    && self.number(v).is_none()
                    && (self.models.contains_key(&self.text(v).to_ascii_lowercase())
                        || kit_model == Some(v)) =>
            {
                model = Some(self.text(v));
                Val::Raw(String::new())
            }
            Some(v) => self.value(v),
            None if e == Element::Capacitor
                && !params.iter().any(|(k, _)| k.eq_ignore_ascii_case("c"))
                && params.iter().any(|(k, _)| k.eq_ignore_ascii_case("q")) =>
            {
                let i = params
                    .iter()
                    .position(|(k, _)| k.eq_ignore_ascii_case("q"))?;
                let pair = pairs[i];
                params.remove(i);
                return Some(Card::Charge {
                    name,
                    n1,
                    n2,
                    charge: self.body_text(self.ast.rhs(Node(pair))),
                    value: self.ast.rhs(Node(pair)),
                    params,
                });
            }
            // Xyce 7.10 runs a resistor with nothing after its nodes at
            // 1000 ohms ("Expected value field for device R2, continuing
            // with value of 0", then "Resistance is set to 0, setting to
            // the default, 1000 ohms"; Xyce_Regression
            // RESISTOR/DefaultValueWarning.cir). A capacitor or an inductor
            // with no value it rejects.
            None if e == Element::Resistor
                && self.b.card_rules == CardRules::Xyce
                && inst.is_empty() =>
            {
                Val::Num(1000.0)
            }
            None => {
                let key = match e {
                    Element::Resistor => "r",
                    Element::Capacitor => "c",
                    _ => "l",
                };
                let i = params
                    .iter()
                    .position(|(k, _)| k.eq_ignore_ascii_case(key))?;
                let pair = pairs[i];
                params.remove(i);
                self.value(self.ast.rhs(Node(pair)))
            }
        };
        let (value, params) = match self.merge_units && lt_cap {
            true => self.merged_units(n, value, params),
            false => (value, params),
        };
        let letter = match e {
            Element::Resistor => 'R',
            Element::Capacitor => 'C',
            _ => 'L',
        };
        Some(Card::Rcl {
            letter,
            name,
            n1,
            n2,
            value,
            model,
            params,
        })
    }
}
