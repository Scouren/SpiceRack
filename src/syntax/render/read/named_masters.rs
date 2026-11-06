//! Spectre built-in masters past the basic nine that have a SPICE card:
//! `iprobe` (a 0 V source), `port` (ngspice's RF port), `tline` (a
//! lossless `T` line), `bsource` with `v=` or `i=` (a `B` source). Any
//! other parameter, or an expression a SPICE `B` source would read with
//! another meaning, gives no card: the statement stays text.
//!
//! Cites: Spectre Circuit Simulator Reference 5.0 `[CMP]` (`iprobe`
//! p.379, `port` p.602, `tline` pp.644-647, `bsource` pp.856-859); ngspice
//! manual §4.1.11 (`portnum`, `z0`: the value is the source behind `z0`),
//! §6.2.1 (`T`), §5.1 (`B`).
//!
//! Contents:
//! - `Rd::named_master(&self, master: &str, name: &str, ns: &[String], p: &[(String, u32)]) -> Option<Card>`

use super::{Card, Rd, Val};
use crate::syntax::ast::{Node, Tag};

/// The functions a Spectre expression and an ngspice `B` source both
/// define, with one meaning (ngspice manual §5.1; `log` and `ln` are
/// left out: the bases differ across the SPICE family).
const SAME_FUNCS: &[&str] = &[
    "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "exp", "sqrt", "abs",
    "min", "max", "pow",
];

impl<'a> Rd<'a> {
    /// An instance (`name (ns) master p`) of a built-in master this file
    /// reads past the basic nine, as its SPICE card; `None` when it is no
    /// such master or has no card.
    pub(in crate::syntax::render) fn named_master(
        &self,
        master: &str,
        name: &str,
        ns: &[String],
        p: &[(String, u32)],
    ) -> Option<Card> {
        // Absent: `Some(None)`; no number: `None`.
        let num = |k: &str| match self.get(p, k) {
            Some(v) => self.number(v).map(Some),
            None => Some(None),
        };
        let only = |keys: &[&str]| p.iter().all(|(k, _)| keys.contains(&k.as_str()));
        match (master, ns) {
            // `[CMP]` p.379: current positive from `in` through the probe
            // to `out`, as through a 0 V source from + to -.
            ("iprobe", [i, o]) if p.is_empty() => Some(Card::Source {
                letter: 'V',
                name: self.strip(name, 'v'),
                np: i.clone(),
                nm: o.clone(),
                value: Val::Num(0.0),
                ac_mag: None,
                ac_phase: None,
                wave: None,
                port: None,
            }),
            // `[CMP]` p.602: `dc` and `mag` are across the port terminated
            // in `r`, so the source behind `r` is twice each; ngspice's
            // value and `ac` are that source's.
            ("port", [a, b]) if only(&["num", "r", "dc", "mag", "phase", "type"]) => {
                if self.get(p, "type").is_some_and(|t| self.text(t) != "dc") {
                    return None;
                }
                let k = self.count(self.get(p, "num")?)?;
                let r = num("r")?.unwrap_or(50.0);
                let dc = num("dc")?.unwrap_or(0.0);
                let mag = num("mag")?;
                Some(Card::Source {
                    letter: 'V',
                    name: self.strip(name, 'v'),
                    np: a.clone(),
                    nm: b.clone(),
                    value: Val::Num(2.0 * dc),
                    ac_mag: mag.map(|m| 2.0 * m),
                    ac_phase: mag.and(num("phase")?),
                    wave: None,
                    port: Some((k, r)),
                })
            }
            // `[CMP]` p.645: the lossless line is `z0` (default 50 Ω) and
            // `td`; `f`/`nl`, `len`/`vel` and the loss terms have no field.
            ("tline", [_, _, _, _]) if only(&["z0", "td"]) => Some(Card::Line {
                name: self.strip(name, 't'),
                nodes: ns.to_vec(),
                z0: num("z0")?.unwrap_or(50.0),
                td: num("td")??,
            }),
            ("bsource", [a, b]) => {
                let [(k, v)] = p else { return None };
                let current = match k.as_str() {
                    "v" => false,
                    "i" => true,
                    _ => return None,
                };
                Some(Card::Behavioural {
                    current,
                    name: self.strip(name, 'b'),
                    np: a.clone(),
                    nm: b.clone(),
                    expr: self.b_expr(*v)?,
                    value: *v,
                })
            }
            _ => None,
        }
    }
}

// ---- private ----

impl<'a> Rd<'a> {
    /// A `bsource` expression `v` in ngspice's `B` spelling (`[CMP]`
    /// p.857: parameters, `$time`, `v(a)`, `v(a,b)`); `None` when it holds
    /// anything else (`ddt`, `idt`, a branch current, a `$` name, a
    /// function whose meaning differs).
    fn b_expr(&self, v: u32) -> Option<String> {
        let a = self.ast;
        let mut work = vec![v];
        // Bounded by the tree: each node is pushed once by its parent.
        while let Some(k) = work.pop() {
            let node = Node(k);
            match a.tag(node) {
                Tag::Num => {}
                Tag::Ident => {
                    let t = self.text(k);
                    if t.starts_with('$') && t != "$time" {
                        return None;
                    }
                }
                Tag::Unary | Tag::Paren => work.push(a.lhs(node)),
                Tag::Binary => work.extend([a.lhs(node), a.rhs(node)]),
                Tag::Ternary => {
                    work.extend([a.lhs(node), a.extra(a.rhs(node)), a.extra(a.rhs(node) + 1)]);
                }
                Tag::Probe | Tag::Call => {
                    let name = self.t.text(a.main_token(node));
                    let at = a.rhs(node);
                    let mut args = (0..a.extra(at + 1)).map(|i| a.extra(at + 2 + i));
                    if name == "v" {
                        // `v(a)`, `v(a,b)`: node names (ground `0` a number).
                        if !args
                            .all(|x| matches!(a.tag(Node(x)), Tag::Ident | Tag::Word | Tag::Num))
                        {
                            return None;
                        }
                    } else if a.tag(node) == Tag::Call && SAME_FUNCS.contains(&name) {
                        work.extend(args);
                    } else {
                        return None;
                    }
                }
                _ => return None,
            }
        }
        Some(self.etext(v).replace("$time", "time"))
    }
}
