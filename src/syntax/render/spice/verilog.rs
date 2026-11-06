//! The deck → structural Verilog: the SPICE writer's Verilog row. Each
//! block (the top, then each subcircuit) is one `module name (ports);` …
//! `endmodule` around its ports (`inout`), the nets its instances use
//! (`wire`), its parameters with a number for a default (`parameter`) and
//! its instances, each `master #(.p(v), …) name (.port(net), …);` paired
//! by the master's ports where the deck defines it, by position
//! otherwise, its numeric parameters by name. Verilog has a module only for
//! a subcircuit: every other element, and text kept as written, is a
//! `// W0029 …` comment in its place, and what else the deck holds (a
//! parameter that is an expression among them) is not written; each is a
//! `W0029` note, the deck still written. A name that is a keyword is
//! escaped; a port named twice is two port expressions on one net.
//!
//! Cites: IEEE 1364-2005 §3.5.1 (reals), §3.7.1 (escaped identifiers: `\`
//! up to a blank), §3.7.2 and Annex B (keywords), §12.1 (module
//! declarations), §12.2 (parameters), §12.2.2.2 (by name), §12.3.1 (port
//! expressions), §12.2.2 (connections by name and by
//! position); the old xschem layout writer, whose
//! element lines this writer does not keep (it wrote the old IR's `Debug`
//! text).
//!
//! Contents:
//! - `W::verilog(&mut self)`

use std::collections::BTreeSet;

use super::W;
use crate::syntax::diag::{Code, info};
use crate::syntax::render::out::Space;
use crate::syntax::render::read::{Block, Card, Comp};

/// IEEE 1364-2005 Annex B, the reserved keywords, sorted (keywords are
/// lowercase and case-sensitive, §3.7.2).
const KEYWORDS: &[&str] = &[
    "always",
    "and",
    "assign",
    "automatic",
    "begin",
    "buf",
    "bufif0",
    "bufif1",
    "case",
    "casex",
    "casez",
    "cell",
    "cmos",
    "config",
    "deassign",
    "default",
    "defparam",
    "design",
    "disable",
    "edge",
    "else",
    "end",
    "endcase",
    "endconfig",
    "endfunction",
    "endgenerate",
    "endmodule",
    "endprimitive",
    "endspecify",
    "endtable",
    "endtask",
    "event",
    "for",
    "force",
    "forever",
    "fork",
    "function",
    "generate",
    "genvar",
    "highz0",
    "highz1",
    "if",
    "ifnone",
    "incdir",
    "include",
    "initial",
    "inout",
    "input",
    "instance",
    "integer",
    "join",
    "large",
    "liblist",
    "library",
    "localparam",
    "macromodule",
    "medium",
    "module",
    "nand",
    "negedge",
    "nmos",
    "nor",
    "noshowcancelled",
    "not",
    "notif0",
    "notif1",
    "or",
    "output",
    "parameter",
    "pmos",
    "posedge",
    "primitive",
    "pull0",
    "pull1",
    "pulldown",
    "pullup",
    "pulsestyle_ondetect",
    "pulsestyle_onevent",
    "rcmos",
    "real",
    "realtime",
    "reg",
    "release",
    "repeat",
    "rnmos",
    "rpmos",
    "rtran",
    "rtranif0",
    "rtranif1",
    "scalared",
    "showcancelled",
    "signed",
    "small",
    "specify",
    "specparam",
    "strong0",
    "strong1",
    "supply0",
    "supply1",
    "table",
    "task",
    "time",
    "tran",
    "tranif0",
    "tranif1",
    "tri",
    "tri0",
    "tri1",
    "triand",
    "trior",
    "trireg",
    "unsigned",
    "use",
    "uwire",
    "vectored",
    "wait",
    "wand",
    "weak0",
    "weak1",
    "while",
    "wire",
    "wor",
    "xnor",
    "xor",
];

impl W<'_, '_> {
    /// The deck, as one module per block.
    pub(super) fn verilog(&mut self) {
        let deck = self.deck;
        self.o.token(&format!("// {}", deck.top.name), Space::Line);
        self.module(&deck.top, true);
        for sc in &deck.defs {
            self.module(sc, false);
        }
        // What a module holds no form for: analyses and outputs, sections,
        // branches, includes, models, functions, loads.
        let mut at: Vec<u32> = deck.sections.iter().map(|s| s.node).collect();
        for b in std::iter::once(&deck.top).chain(&deck.defs) {
            at.extend(b.conds.iter().map(|&(n, _)| n));
            at.extend(b.includes.iter().chain(&b.libs).chain(&b.loads));
            at.extend(b.models.iter().map(|&(m, _)| m));
            at.extend(b.params.iter().chain(&b.locals));
        }
        if let Some(&first) = at.first() {
            self.note(Code::W0029, self.rd.main(first));
        }
        if !deck.bench.is_empty() || !deck.funcs.is_empty() {
            self.note(Code::W0029, 0);
        }
    }
}

// ---- private ----

impl W<'_, '_> {
    /// One block as a module: its ports, the nets its instances use, its
    /// instances, a comment per element no module stands for.
    fn module(&mut self, b: &Block, top: bool) {
        let (ports, params) = match top {
            true => (Vec::new(), Vec::new()),
            false => {
                let head = self.rd.subckt_head(b.node);
                let head = head.expect("the reader read the head");
                (head.ports, head.params)
            }
        };
        let pins = pin_names(&ports);
        // A port named twice is one net under two port names: a port
        // expression each (§12.3.1, `.name(net)`).
        let list: Vec<String> = match pins.iter().zip(&ports).all(|(p, n)| p == n) {
            true => pins.iter().map(|p| ident(p)).collect(),
            false => pins
                .iter()
                .zip(&ports)
                .map(|(p, n)| format!(".{}({})", ident(p), ident(n)))
                .collect(),
        };
        // The deck's own module is named after its title, which a
        // subcircuit may be named too: modules share one name space.
        let taken = top && self.deck.defs.iter().any(|d| d.name == b.name);
        let name = match taken {
            true => format!("{}_top", b.name),
            false => b.name.clone(),
        };
        let head = format!("module {} ({});", ident(&name), list.join(", "));
        self.o.token(&head, Space::Line);
        let mut nets: Vec<&String> = Vec::new();
        for p in &ports {
            if !nets.contains(&p) {
                nets.push(p);
                self.o.token(&format!("  inout {};", ident(p)), Space::Line);
            }
        }
        // §12.2: a parameter an instance overrides is the module's, with a
        // default; one whose default is no number is not written.
        for (k, v) in &params {
            match real(v) {
                Some(r) => {
                    let line = format!("  parameter {} = {r};", ident(k));
                    self.o.token(&line, Space::Line);
                }
                None => self.note(Code::W0029, self.rd.main(b.node)),
            }
        }
        let insts: Vec<Card> = b.insts.iter().map(|&(_, n)| self.rd.card_view(n)).collect();
        let mut wires = BTreeSet::new();
        for i in &insts {
            if let Card::Inst { ports: nets, .. } = i {
                wires.extend(nets.iter().filter(|n| !ports.contains(n) && *n != "0"));
            }
        }
        for w in wires {
            self.o.token(&format!("  wire {};", ident(w)), Space::Line);
        }
        for (i, &(_, n)) in insts.iter().zip(&b.insts) {
            let (line, all) = self.instance(i);
            self.o.token(&line, Space::Line);
            if !all {
                self.note(Code::W0029, self.rd.main(n));
            }
        }
        let title = info(Code::W0029).title;
        for &c in &b.comps {
            let (Comp::Card(n) | Comp::Raw(n)) = c;
            self.note(Code::W0029, self.rd.main(n));
            let name = self.rd.t.text(self.rd.main(n));
            self.o
                .token(&format!("  // W0029 {title}: {name}"), Space::Line);
        }
        self.o.token("endmodule", Space::Line);
    }

    /// `master #(.p(v), …) name (.port(net), …);`, by name where the deck
    /// defines the master, else `master name (net, …);`; and whether every
    /// parameter was written (a value that is no number is not, nor one
    /// the deck's master declares no number for).
    fn instance(&self, i: &Card) -> (String, bool) {
        let Card::Inst {
            name,
            ports: nets,
            master,
            params,
        } = i
        else {
            unreachable!("an instance the reader placed");
        };
        let def = self
            .deck
            .defs
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(master));
        let head = def.and_then(|d| self.rd.subckt_head(d.node));
        let pins = head.as_ref().map(|h| pin_names(&h.ports));
        let conns: Vec<String> = match pins.filter(|p| p.len() == nets.len()) {
            Some(pins) => pins
                .iter()
                .zip(nets)
                .map(|(p, n)| format!(".{}({})", ident(p), ident(n)))
                .collect(),
            None => nets.iter().map(|n| ident(n)).collect(),
        };
        // §12.2.2.2: parameters by name, each a real, named as the deck's
        // master declares it (Verilog names are case-sensitive, SPICE's
        // are not); one it declares no number for is not written.
        let declared = |k: &str| match &head {
            None => Some(k.to_string()),
            Some(h) => h
                .params
                .iter()
                .find(|(p, v)| p.eq_ignore_ascii_case(k) && real(v).is_some())
                .map(|(p, _)| p.clone()),
        };
        let set: Vec<String> = params
            .iter()
            .filter_map(|(k, v)| Some(format!(".{}({})", ident(&declared(k)?), real(v)?)))
            .collect();
        let over = match set.is_empty() {
            true => String::new(),
            false => format!(" #({})", set.join(", ")),
        };
        // The view's name follows the card's letter; the instance keeps it.
        let name = format!("X{name}");
        let line = format!(
            "  {}{over} {} ({});",
            ident(master),
            ident(&name),
            conns.join(", ")
        );
        (line, set.len() == params.len())
    }
}

/// The port names of a port list, each unique: a name the list holds
/// already gains `_1`, `_2`, … (the net stays the port's).
fn pin_names(ports: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::with_capacity(ports.len());
    for p in ports {
        let taken = |w: &String| out.contains(w) || (w != p && ports.contains(w));
        let w = std::iter::once(p.clone())
            .chain((1..=ports.len()).map(|i| format!("{p}_{i}")))
            .find(|w| !taken(w))
            .expect("a free name among len + 1");
        out.push(w);
    }
    out
}

/// A parameter value as a Verilog real (§3.5.1: `1e-06`), when it is a
/// number (SPICE suffixes and a unit after it read); `None` for an
/// expression.
fn real(v: &str) -> Option<String> {
    let t = v.trim().trim_matches(['{', '}', '\'', '"']);
    let body = t.strip_prefix(['+', '-']).unwrap_or(t);
    let (x, len, _) = crate::unit::parse_spice_number(body)?;
    if !body[len..].bytes().all(|c| c.is_ascii_alphabetic()) || !x.is_finite() {
        return None;
    }
    let x = if t.starts_with('-') { -x } else { x };
    Some(format!("{x:e}"))
}

/// `name` as a Verilog identifier: as is when it is a simple one
/// (`[A-Za-z_][A-Za-z0-9_$]*`) and no keyword, else escaped (`\name `,
/// IEEE 1364-2005 §3.7.1, §3.7.2).
fn ident(name: &str) -> String {
    let b = name.as_bytes();
    let simple = b
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && b.iter()
            .all(|c| c.is_ascii_alphanumeric() || *c == b'_' || *c == b'$');
    match simple && KEYWORDS.binary_search(&name).is_err() {
        true => name.to_string(),
        false => format!("\\{name} "),
    }
}

#[cfg(test)]
mod tests {
    use crate::syntax::Dialect;
    use crate::syntax::diag::Code;
    use crate::syntax::render::render_text;

    #[test]
    fn keywords_parameters_and_a_twice_named_port() {
        // §3.7.2: `primitive` and `output` are keywords, escaped (§3.7.1).
        // §12.2: `w` is the module's parameter, its default a real; §12.2.2.2
        // `#(.w(2e-6))` overrides it, the expression `{w*2}` is no number
        // (W0029). §12.3.1: `.subckt d a a` is ports `.a(a)`, `.a_1(a)`.
        // Checks: 4.
        let deck = "t\n.subckt primitive output w=1u\nR1 output 0 1k\n.ends\n.subckt d a a\n.ends\nX1 n primitive w=2u\nX2 n primitive w={w*2}\nX3 m m d\n.op\n.end\n";
        let w = render_text(deck, Dialect::Ngspice, "t", Dialect::Verilog).expect("written");
        assert!(
            w.text.contains(
                "module \\primitive  (\\output );\n  inout \\output ;\n  parameter w = 1e-6;\n"
            ),
            "{}",
            w.text
        );
        assert!(
            w.text
                .contains("  \\primitive  #(.w(2e-6)) X1 (.\\output (n));\n"),
            "{}",
            w.text
        );
        assert!(
            w.text.contains("  \\primitive  X2 (.\\output (n));\n"),
            "{}",
            w.text
        );
        assert!(
            w.text.contains("module d (.a(a), .a_1(a));\n  inout a;\n"),
            "{}",
            w.text
        );
    }

    #[test]
    fn the_deck_module_yields_its_name_to_a_subcircuit() {
        // §12.1: module names are one name space; the deck `t` defining a
        // subcircuit `t` is `t_top`. Checks: 1.
        let deck = "t\n.subckt t a\n.ends\nX1 n t\n.op\n.end\n";
        let w = render_text(deck, Dialect::Ngspice, "t", Dialect::Verilog).expect("written");
        assert!(w.text.starts_with("// t\nmodule t_top ();\n"), "{}", w.text);
    }

    #[test]
    fn the_keyword_table_is_sorted() {
        assert!(super::KEYWORDS.is_sorted(), "binary search needs it");
    }

    #[test]
    fn subcircuits_are_modules_and_instances_connect_by_port() {
        // IEEE 1364-2005 §12.2.2: `inv u1 (.a(n1), .y(out));` connects by
        // name; a master the deck does not define connects by position.
        // A ground net `0` and a net `n<1>` are escaped (§3.7.1). Checks: 3.
        let deck = "t\n.subckt inv a y\nM1 y a 0 0 nmos\n.ends\nX1 in n<1> inv\nX2 n<1> out pdk_cell\nR1 out 0 1k\n.op\n.end\n";
        let w = render_text(deck, Dialect::Ngspice, "t", Dialect::Verilog).expect("written");
        let want = "// t\nmodule t ();\n  wire in;\n  wire \\n<1> ;\n  wire out;\n  inv X1 (.a(in), .y(\\n<1> ));\n  pdk_cell X2 (\\n<1> , out);\n  // W0029 a statement structural Verilog does not write: R1\nendmodule\nmodule inv (a, y);\n  inout a;\n  inout y;\n  // W0029 a statement structural Verilog does not write: M1\nendmodule\n";
        assert_eq!(w.text, want);
        let codes: Vec<Code> = w.notes.diags.iter().map(|d| d.code).collect();
        assert!(codes.iter().all(|&c| c == Code::W0029), "{codes:?}");
        assert_eq!(codes.len(), 3, "R1, M1, the analysis");
    }
}
