//! A Verilog-A source's text → its module headers: each module's name, its
//! ports in header order (ANSI or not) and its `parameter` names. A token
//! scan, not a parser: comments, attributes `(* … *)`, strings and
//! `` ` `` directive lines are skipped; a `localparam` and every default
//! value are never read. Ported from Interop's `pdk_scan` (`va_modules`,
//! `va_tokens`).
//!
//! Cites: IEEE 1364-2005 §12.1 (module declarations, ANSI and non-ANSI port
//! lists), §4.10.1 (`parameter`), §4.10.2 (`localparam`); Verilog-AMS LRM
//! 2.4 §3.4 (`parameter … from`).
//!
//! Contents:
//! - `pub(super) struct Module`
//! - `pub(super) fn modules(text: &str) -> Vec<Module>`

/// One module's header.
#[derive(Debug, PartialEq)]
pub(super) struct Module {
    pub(super) name: String,
    pub(super) ports: Vec<String>,
    pub(super) params: Vec<String>,
}

/// Every module `text` declares and closes with `endmodule`, in source
/// order.
///
/// Cost: one pass over `text`, its tokens held as owned words.
pub(super) fn modules(text: &str) -> Vec<Module> {
    let toks = tokens(text);
    let at = |i: usize| toks.get(i).map_or("", String::as_str);
    let mut out = Vec::new();
    let mut cur: Option<Module> = None;
    let mut i = 0;
    while i < toks.len() {
        match at(i) {
            "module" | "macromodule" if is_ident(at(i + 1)) => {
                let mut m = Module {
                    name: toks[i + 1].clone(),
                    ports: Vec::new(),
                    params: Vec::new(),
                };
                i += 2;
                if at(i) == "(" {
                    // ANSI or not, a port is the last word before each `,`
                    // and the `)`.
                    let mut last = None;
                    i += 1;
                    while i < toks.len() && at(i) != ")" {
                        match at(i) {
                            "," => m.ports.extend(last.take()),
                            t if is_ident(t) => last = Some(t.to_owned()),
                            _ => {}
                        }
                        i += 1;
                    }
                    m.ports.extend(last);
                }
                cur = Some(m);
            }
            // `parameter [type] name = value [from …] {, name = value} ;`
            "parameter" => {
                let (mut depth, mut want) = (0i32, true);
                i += 1;
                while i < toks.len() && !(depth == 0 && at(i) == ";") {
                    match at(i) {
                        "(" | "[" | "{" => depth += 1,
                        ")" | "]" | "}" => depth -= 1,
                        "," if depth == 0 => want = true,
                        t if want && depth == 0 && is_ident(t) && at(i + 1) == "=" => {
                            if let Some(m) = &mut cur {
                                m.params.push(t.to_owned());
                            }
                            want = false;
                        }
                        _ => {}
                    }
                    i += 1;
                }
            }
            "endmodule" => out.extend(cur.take()),
            _ => {}
        }
        i += 1;
    }
    assert!(out.iter().all(|m| is_ident(&m.name)), "a module is named");
    out
}

// ---- private ----

fn is_ident(t: &str) -> bool {
    t.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_' || c == '\\')
}

/// `text` as words and one-character punctuation, without comments,
/// attributes, strings or compiler directives (a `` ` `` line).
fn tokens(text: &str) -> Vec<String> {
    let skip_to = |from: usize, end: &str| {
        text.get(from..)
            .and_then(|t| t.find(end))
            .map_or(text.len(), |k| from + k + end.len())
    };
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
    let (mut out, mut i) = (Vec::new(), 0);
    while let Some(c) = text[i..].chars().next() {
        let rest = &text[i..];
        if rest.starts_with("//") || c == '`' {
            i = skip_to(i, "\n");
        } else if rest.starts_with("/*") {
            i = skip_to(i + 2, "*/");
        } else if rest.starts_with("(*") && !rest.starts_with("(*)") {
            i = skip_to(i + 2, "*)");
        } else if c == '"' {
            i = skip_to(i + 1, "\"");
        } else if word(c) || c == '\\' {
            // An escaped identifier runs to white space.
            let end = rest[1..]
                .find(|d: char| {
                    if c == '\\' {
                        d.is_whitespace()
                    } else {
                        !word(d)
                    }
                })
                .map_or(rest.len(), |k| k + 1);
            out.push(rest[..end].to_owned());
            i += end;
        } else {
            if !c.is_whitespace() {
                out.push(c.to_string());
            }
            i += c.len_utf8();
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers_ansi_and_not_skipping_comments_attributes_and_localparams() {
        // IEEE 1364-2005 §12.1: `module m(inout electrical p, …)` (ANSI) and
        // `module m (a, b, c); inout a, b, c;` declare the same port order;
        // §4.10.2: a `localparam` is no parameter. Interop's fixture
        // `pdk_scan/spectre/va/varactor.va`, read by hand. Checks: 1.
        let va = "`include \"disciplines.vams\"\n\
            /* module fake(x); */\n\
            module varactor_va(inout electrical p, inout electrical n);\n\
            (* desc=\"area (k=9)\" *) parameter real area = 4.56e-12 from (0:inf);\n\
            parameter real c0 = 7.89e-15, vbi = 0.654\n from [0:inf);\n\
            parameter integer lvl = 3;\n\
            localparam real hidden = 0.111;\n\
            analog I(p, n) <+ ddt(c0 * area * V(p, n) / vbi);\n\
            endmodule\n\
            module twoport (a, b, c)\n ; // split\n inout a, b, c;\n\
            parameter real g = 1.23e-3;\nendmodule\n\
            module open(x);\n";
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        assert_eq!(
            modules(va),
            [
                Module {
                    name: "varactor_va".into(),
                    ports: s(&["p", "n"]),
                    params: s(&["area", "c0", "vbi", "lvl"]),
                },
                Module {
                    name: "twoport".into(),
                    ports: s(&["a", "b", "c"]),
                    params: s(&["g"]),
                },
            ]
        );
    }
}
