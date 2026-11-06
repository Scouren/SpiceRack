//! Spectre and VACASK instance statements → [`Card`] views (by built-in
//! master or model kind, or a subcircuit instance), with their source
//! waveforms (`named_waves`), as the old lowering typed them; a master another file of the
//! kit declares is read as `syntax::resolve` binds it.
//!
//! Cites: the old lowering's named elements;
//! Spectre Circuit Simulator Reference 5.0 `[CMP]` (`vsource` #4, #17, #18,
//! p.686); the old Spectre and VACASK writers (the key names, inverted).
//!
//! Contents:
//! - `fn unquoted(v: &str) -> Option<String>`
//! - `Rd::named_inst(&self, n: u32) -> Option<Card>`, `Rd::velems(&self, v: u32) -> Vec<u32>`,
//!   `Rd::va_module(&self, n: u32) -> Option<String>`, `Rd::strip(&self, name: &str, letter: char) -> String`

use super::named::{OSDI, spectre_kind, vacask_model_kind};
use super::{Card, Rd, Val};
use crate::syntax::ast::{NONE, Node, Tag};
use crate::syntax::behaviour::NamedRules;
use crate::syntax::resolve::{Binding, Def, DefKind, Polarity};

/// The keys one dialect's built-in masters use (the old lowering).
struct Keys {
    source: &'static [&'static str],
    gm: &'static str,
    rm: &'static str,
    probe: &'static str,
    mutual: &'static str,
    coupling: &'static str,
}

const SPECTRE: Keys = Keys {
    source: &[
        "dc",
        "mag",
        "phase",
        "type",
        "val0",
        "val1",
        "delay",
        "rise",
        "fall",
        "width",
        "period",
        "sinedc",
        "ampl",
        "freq",
        "damp",
        "sinephase",
        "fmmodindex",
        "fmmodfreq",
        "td1",
        "tau1",
        "td2",
        "tau2",
        "wave",
        "pwlperiod",
    ],
    gm: "gm",
    rm: "rm",
    probe: "probe",
    mutual: "mutual_inductor",
    coupling: "coupling",
};

const VACASK: Keys = Keys {
    source: &[
        "dc", "mag", "phase", "type", "val0", "val1", "delay", "rise", "fall", "width", "period",
        "sinedc", "ampl", "freq", "theta", "modindex", "modfreq", "tau1", "td2", "tau2", "wave",
    ],
    gm: "gain",
    rm: "gain",
    probe: "ctlinst",
    mutual: "mutual",
    coupling: "k",
};

/// What the kit declares a master as.
enum Declared {
    Subckt,
    /// A model, by the kind `Rd::declarations` would give it.
    Model(String),
}

/// `"text"` without its quotes.
pub(in crate::syntax::render) fn unquoted(v: &str) -> Option<String> {
    Some(v.strip_prefix('"')?.strip_suffix('"')?.to_string())
}

impl<'a> Rd<'a> {
    /// One instance statement as a typed card or a subcircuit instance;
    /// `None` when the IR had no form for it (the old lowering's `inst`).
    pub(in crate::syntax::render) fn named_inst(&self, n: u32) -> Option<Card> {
        let (kinds, subckts) = (&self.kinds, &self.subckts);
        let keys = self.keys();
        let h = self.ast.header(Node(n));
        let name = self.name_of(h.name);
        let ns: Vec<String> = h.nodes.iter().map(|&k| self.name_of(k)).collect();
        let m = h.master;
        if m == NONE {
            return None;
        }
        let mut master = self.name_of(m);
        // A master this file does not declare: the kit's declaration.
        let local = kinds.contains_key(&master) || subckts.contains(&master);
        let kit = if local { None } else { self.kit_master(n) };
        let kind_of = match &kit {
            Some(Declared::Model(k)) => Some(k.clone()),
            Some(Declared::Subckt) | None => kinds.get(&master).cloned(),
        };
        if let Some(b) = kind_of.as_deref().and_then(|k| k.strip_prefix('=')) {
            master = b.to_string();
        }
        let p = self.pairs(n);
        let kept = |used: &[&str]| -> Vec<(String, String)> {
            p.iter()
                .filter(|(k, _)| !used.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), self.vtext(*v)))
                .collect()
        };
        // A device's parameters: VACASK's `$mfactor` (N in parallel,
        // `docs/cir-mfactor.md`) is SPICE's multiplier `m` (ngspice manual
        // §3.3.1, §7.1, §11.1); VACASK's build of a SPICE device names an
        // instance parameter that shares a model parameter's name
        // `instance_<p>` (`devices/spice/bsim3v3.va`, `bsim4v8.va`
        // headers), SPICE's `<p>`.
        let device = |used: &[&str]| -> Vec<(String, String)> {
            let mut v = kept(used);
            for (k, _) in &mut v {
                if self.b.named_rules != NamedRules::Vacask {
                    continue;
                }
                if k == "$mfactor" {
                    *k = "m".into();
                } else if let Some(p) = k.strip_prefix("instance_") {
                    *k = p.to_string();
                }
            }
            v
        };
        // Whether every parameter is one of `known` (a card with no field
        // for another would drop it).
        let only = |known: &[&str]| p.iter().all(|(k, _)| known.contains(&k.as_str()));
        let two = || match ns.as_slice() {
            [a, b] => Some((a.clone(), b.clone())),
            _ => None,
        };
        let num = |k: &str| self.get(&p, k).and_then(|v| self.number(v));
        let ident = |k: &str| {
            self.get(&p, k).map(|v| {
                let t = self.text(v);
                unquoted(&t).unwrap_or(t)
            })
        };
        Some(match master.as_str() {
            "resistor" | "capacitor" | "inductor" => {
                let (n1, n2) = two()?;
                let (letter, key) = match master.as_str() {
                    "resistor" => ('r', "r"),
                    "capacitor" => ('c', "c"),
                    _ => ('l', "l"),
                };
                Card::Rcl {
                    letter: letter.to_ascii_uppercase(),
                    name: self.strip(&name, letter),
                    n1,
                    n2,
                    value: self.value(self.get(&p, key)?),
                    model: None,
                    params: device(&[key]),
                }
            }
            "vsource" | "isource" => {
                let (np, nm) = two()?;
                if p.iter().any(|(k, _)| !keys.source.contains(&k.as_str())) {
                    return None;
                }
                let wave = self.named_waveform(&p)?;
                // VACASK: `dc` is `type="dc"`'s value; a waveform's DC value
                // is its value at t = 0 (`docs/dev-builtin-src.md`; a VACASK
                // op of `dc=5 type="pulse" val0=1` is 1).
                let value = match (&wave, self.b.named_rules) {
                    (Some(w), NamedRules::Vacask) => w.at_zero(),
                    _ => self.get(&p, "dc").map_or(Val::Num(0.0), |v| self.value(v)),
                };
                let letter = if master == "vsource" { 'v' } else { 'i' };
                Card::Source {
                    letter: letter.to_ascii_uppercase(),
                    name: self.strip(&name, letter),
                    np,
                    nm,
                    value,
                    ac_mag: num("mag"),
                    ac_phase: num("phase"),
                    wave,
                    port: None,
                    parasitics: vec![],
                }
            }
            "vcvs" | "vccs" => {
                let [np, nm, ncp, ncm] = ns.as_slice() else {
                    return None;
                };
                if !only(&["gain", keys.gm]) {
                    return None;
                }
                let (letter, gain) = match master.as_str() {
                    "vcvs" => ('e', num("gain")?),
                    _ => ('g', num(keys.gm)?),
                };
                Card::Linear {
                    letter: letter.to_ascii_uppercase(),
                    name: self.strip(&name, letter),
                    nodes: vec![np.clone(), nm.clone(), ncp.clone(), ncm.clone()],
                    gain,
                }
            }
            "cccs" | "ccvs" => {
                let (np, nm) = two()?;
                if !only(&["gain", keys.rm, keys.probe]) {
                    return None;
                }
                // The controlling source as the name map spells its
                // declaration (`in1` is `vin1`); none: no SPICE card.
                let vsense = self.declared(n, &ident(keys.probe)?, &['V'])?;
                let (letter, gain) = match master.as_str() {
                    "cccs" => ('f', num("gain")?),
                    _ => ('h', num(keys.rm)?),
                };
                Card::Linear {
                    letter: letter.to_ascii_uppercase(),
                    name: self.strip(&name, letter),
                    nodes: vec![np, nm, vsense],
                    gain,
                }
            }
            m if m == keys.mutual
                && (self.b.named_rules == NamedRules::Spectre || ns.is_empty())
                && only(&["ind1", "ind2", keys.coupling]) =>
            {
                let refs = vec![ident("ind1")?, ident("ind2")?];
                Card::Coupling {
                    name: self.strip(&name, 'k'),
                    ls: refs.iter().map(|r| self.strip(r, 'l')).collect(),
                    refs,
                    k: num(keys.coupling)?,
                }
            }
            "pvcvs" | "pvccs" | "pcccs" | "pccvs" if self.b.named_rules == NamedRules::Spectre => {
                let letter = match master.as_str() {
                    "pvcvs" => 'E',
                    "pvccs" => 'G',
                    "pcccs" => 'F',
                    _ => 'H',
                };
                let list = |k: &str| self.get(&p, k).map(|v| self.velems(v));
                let coeffs: Vec<f64> = list("coeffs")?
                    .into_iter()
                    .map(|c| self.number(c))
                    .collect::<Option<_>>()?;
                let (np, nm) = (ns.first()?.clone(), ns.get(1)?.clone());
                let controls = if matches!(letter, 'E' | 'G') {
                    ns[2..].to_vec()
                } else {
                    list("probes")?
                        .into_iter()
                        .map(|c| self.name_of(c))
                        .collect()
                };
                Card::Poly {
                    name: self.strip(&name, letter.to_ascii_lowercase()),
                    letter,
                    np,
                    nm,
                    controls,
                    coeffs,
                }
            }
            "iprobe" | "port" | "tline" | "bsource"
                if self.b.named_rules == NamedRules::Spectre
                    && kit.is_none()
                    && !subckts.contains(&master) =>
            {
                return self.named_master(&master, &name, &ns, &p);
            }
            // A Verilog-A module the kit loads, instantiated by name with
            // its parameters (Spectre `ahdl_include`): ngspice's `N` card,
            // the writer giving it a model of its own (`Rd::va_module`).
            m if self.va_module(n).is_some() => Card::Semi {
                letter: 'N',
                name: name.clone(),
                nodes: ns,
                model: m.to_string(),
                params: kept(&[]),
            },
            m if subckts.iter().any(|s| s == m) || matches!(kit, Some(Declared::Subckt)) => {
                return Some(Card::Inst {
                    name: self.strip(&name, 'x'),
                    ports: ns,
                    master: self
                        .masters
                        .get(&n)
                        .cloned()
                        .unwrap_or_else(|| m.to_string()),
                    params: kept(&[]),
                });
            }
            // A loaded module's instance: ngspice's OSDI `N` card (manual
            // §13: an instance of a `.model` of the module), named as read.
            m if kind_of.as_deref().is_some_and(|k| k.starts_with(OSDI)) => Card::Semi {
                letter: 'N',
                name: name.to_lowercase(),
                nodes: ns,
                model: m.to_string(),
                // A loaded module's own parameter names, `$mfactor` its `m`.
                params: kept(&[])
                    .into_iter()
                    .map(|(k, v)| match k.as_str() {
                        "$mfactor" => ("m".into(), v),
                        _ => (k, v),
                    })
                    .collect(),
            },
            m => {
                let kind = kind_of.as_deref()?;
                let (model, params) = (m.to_string(), device(&[]));
                let letter = match (kind, ns.len()) {
                    ("d", 2) => 'd',
                    ("npn" | "pnp", 3..=5) => 'q',
                    ("nmos" | "pmos", 4) => 'm',
                    ("njf" | "pjf", 3) if self.b.named_rules == NamedRules::Vacask => 'j',
                    ("nmf" | "pmf", 3) if self.b.named_rules == NamedRules::Vacask => 'z',
                    _ => return None,
                };
                Card::Semi {
                    letter: letter.to_ascii_uppercase(),
                    name: self.strip(&name, letter),
                    nodes: ns,
                    model,
                    params,
                }
            }
        })
    }

    /// A vector's element rows, each with its sign (`-1` is a `Unary` row
    /// whose number is -1; `[CMP]` `vsource` `wave`).
    pub(super) fn velems(&self, v: u32) -> Vec<u32> {
        if self.tag(v) != Tag::Vector {
            return vec![v];
        }
        self.ast.list(self.ast.rhs(Node(v)) + 1).to_vec()
    }

    /// The Verilog-A module instance `n` names directly, when the kit
    /// binds its master to one (`DefKind::VerilogA`): no model names it.
    pub(in crate::syntax::render) fn va_module(&self, n: u32) -> Option<String> {
        if self.tag(n) != Tag::Instance {
            return None;
        }
        let d = self.kit_def(n)?;
        matches!(d.kind, DefKind::VerilogA { .. }).then(|| d.name.clone())
    }

    /// `name`'s SPICE spelling: the letter a writer adds back is dropped
    /// (the old lowering's `strip`).
    pub(super) fn strip(&self, name: &str, letter: char) -> String {
        let name = if self.b.named_rules == NamedRules::Vacask {
            name.to_lowercase()
        } else {
            name.to_string()
        };
        match name.chars().next() {
            Some(c) if c.eq_ignore_ascii_case(&letter) && name.len() > 1 => {
                name[c.len_utf8()..].to_string()
            }
            _ => name,
        }
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// What instance `n`'s master is declared as in the kit, its kind
    /// named as `Rd::declarations` names a model's (the old lowering's
    /// `spectre_kind`, `vacask_kind`, `vacask_builtin`).
    fn kit_master(&self, n: u32) -> Option<Declared> {
        match &self.kit_def(n)?.kind {
            DefKind::Subckt { .. } => Some(Declared::Subckt),
            DefKind::Model { word, polarity, .. } => {
                let p = *polarity == Some(Polarity::P);
                if self.b.named_rules == NamedRules::Vacask {
                    let ty = polarity.map(|_| if p { -1.0 } else { 1.0 });
                    return Some(Declared::Model(vacask_model_kind(word, ty)));
                }
                let ty = p.then_some(if word == "bjt" { "pnp" } else { "p" });
                Some(Declared::Model(spectre_kind(word, ty).0))
            }
            // A Verilog-A module is written as an unknown master is.
            DefKind::Param | DefKind::Func | DefKind::Section | DefKind::VerilogA { .. } => None,
        }
    }

    /// The declaration instance `n`'s master binds to in the kit.
    fn kit_def(&self, n: u32) -> Option<&'a Def> {
        let (kit, f) = self.kit?;
        match kit.binding(f, Node(n))? {
            Binding::Def(id) => Some(kit.definition(id)),
            Binding::Primitive(_) | Binding::Unknown => None,
        }
    }

    fn keys(&self) -> &'static Keys {
        if self.b.named_rules == NamedRules::Vacask {
            &VACASK
        } else {
            &SPECTRE
        }
    }
}
