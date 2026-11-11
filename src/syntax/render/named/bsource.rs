//! A behavioural source's expression tree → the target's behavioural
//! expression: Spectre's `bsource` (`b1 (p n) bsource v=expr`) or VACASK's
//! source line (`b1 (p n) v=expr`, no master), written from the tree
//! post-order on a heap stack, every compound operand parenthesised so
//! the target's precedence and associativity (`**` right-associative, unary
//! minus above it) cannot regroup it. A form the target has no spelling
//! for with the same meaning is `None`, refused W0102.
//!
//! What maps, and its oracle:
//! - `v(a)`, `v(a,b)` as written; `i(V1)` is `i("v1:0")`, the source's
//!   port current (`[CMP]` p.857, the `cccs1` example p.860).
//! - `time` is `$time`, `temper` is `temp` (Celsius, `[CMP]` p.884),
//!   `pi` is `M_PI` (p.864); `hertz` has no bsource form.
//! - `+ - * /`, comparisons, `&& ||`, `?:` as written (pp.867-868);
//!   ngspice's `!x` is `(x)==0` (Spectre has no `!`; both give 1 or 0).
//! - The functions both define alike (pp.869-870); `ln` is `log`. ngspice's
//!   `x**y`, `x^y` and `pow(x,y)` are `|x|^y` (ngspice-45, measured:
//!   `(-2)**3`, `(-2)^3`, `pow(-2,3)` are 8), so `pow(abs(x),y)`; the
//!   other dialects' power is not written (no oracle here for a negative
//!   base), nor Xyce's `log` (its base not checked).
//! - A call of one of the deck's own functions as written (`real f()`).
//!
//! VACASK (`docs/dev-builtin-behavioral.md`, `docs/expr-operators.md`;
//! each row measured against ngspice-45, ZERO-LEDGER P4-002) takes the
//! same tree with these spellings its own:
//! - `time` (a Spectre deck's `$time`) is `$abstime`, the temperature is
//!   `$temp` (Celsius); `M_PI` is its constant too.
//! - `i(V1)` is `i(v1)`, a voltage source's branch current with ngspice's
//!   sign; a device with no branch (a resistor) has none to read.
//! - A number, a node and a parameter name as the rest of the VACASK deck
//!   spells them (`vacask::num`, `vacask::q`, lowercase unless the deck is
//!   VACASK's own).
//! - Its `log` is the natural logarithm, as Spectre's; the operators and
//!   the functions above are the same.
//!
//! Depth: the parser caps an expression's nesting at `DEPTH_MAX` (P0006);
//! the walk is a heap stack regardless.
//!
//! Cites: Spectre Reference 5.0 `[CMP]` pp.856-860 (bsource), pp.864
//! (constants), pp.867-870 (expressions, functions); ngspice manual §5.1
//! (B source functions); VACASK 36be560 `docs/dev-builtin-behavioral.md`.
//!
//! Contents:
//! - `W::bsource_card(&mut self, tok, own, current, nodes, expr, value) -> Option<String>`
//! - `W::bsource_expr(&self, value: u32) -> Option<String>`

use super::W;
use super::vacask::{num, q};
use crate::syntax::ast::{NONE, Name, Node, Tag};
use crate::syntax::behaviour::{CardRules, ModelSemantics, NamedRules};
use crate::syntax::diag::Code;
use crate::syntax::render::capability::{Action, Construct};

/// Functions SPICE and Spectre define with the same meaning (`[CMP]`
/// pp.869-870; ngspice manual §5.1).
const SAME: &[&str] = &[
    "abs", "sqrt", "exp", "log10", "sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh",
    "tanh", "asinh", "acosh", "atanh", "min", "max", "floor", "ceil",
];

/// One step of the walk: a node to open, or a node whose operands are on
/// the value stack.
enum Step {
    Open(u32),
    Close(u32),
}

impl W<'_, '_> {
    /// Value tree `value` as the target's behavioural expression (Spectre's
    /// `bsource`, VACASK's `v=`/`i=`); `None` when a form in it has no
    /// spelling there with its meaning.
    pub(super) fn bsource_expr(&self, value: u32) -> Option<String> {
        let a = self.rd.ast;
        let ngspice = self.rd.b.model_semantics == ModelSemantics::Ngspice;
        let vacask = self.b.named_rules == NamedRules::Vacask;
        let own: Vec<String> = self
            .deck
            .funcs
            .iter()
            .map(|&f| self.rd.func_view(f).0.to_ascii_lowercase())
            .collect();
        // Each value: its text and whether it needs parentheses as an
        // operand.
        let mut vals: Vec<(String, bool)> = Vec::new();
        let mut work = vec![Step::Open(value)];
        while let Some(step) = work.pop() {
            match step {
                Step::Open(n) => {
                    if n == NONE {
                        return None;
                    }
                    let node = Node(n);
                    match a.tag(node) {
                        Tag::Num => {
                            let v = self.rd.t.value(a.main_token(node));
                            // VACASK's number spelling is its deck's
                            // (`meg`, never Spectre's `M`): `vacask::num`.
                            let s = if vacask { num(v) } else { self.num(v) };
                            vals.push((s, false));
                        }
                        Tag::Ident | Tag::Word => {
                            let name = self.rd.text(n);
                            let s = match name.to_ascii_lowercase().as_str() {
                                "time" | "$time" if vacask => "$abstime".to_string(),
                                "time" => "$time".to_string(),
                                // Spectre's `temp`, VACASK's `$temp`: Celsius.
                                "temper" => self.b.temp_name.to_string(),
                                t if vacask && t == self.rd.b.temp_name => {
                                    self.b.temp_name.to_string()
                                }
                                "pi" => "M_PI".to_string(),
                                "hertz" => return None,
                                // VACASK's names are case-sensitive and the
                                // deck's are written lowercase (`W::expr`).
                                low if vacask && !self.keep_case => low.trim().to_string(),
                                _ => name.trim().to_string(),
                            };
                            vals.push((s, false));
                        }
                        Tag::Paren | Tag::Delimited => work.push(Step::Open(a.lhs(node))),
                        Tag::Probe | Tag::Call => {
                            let fname = a.name(Name(a.lhs(node))).to_ascii_lowercase();
                            let args = a.list(a.rhs(node) + 1);
                            match fname.as_str() {
                                "v" | "i" => vals.push((self.probe(&fname, args)?, false)),
                                _ => {
                                    work.push(Step::Close(n));
                                    work.extend(args.iter().rev().map(|&k| Step::Open(k)));
                                }
                            }
                        }
                        Tag::Unary => work.extend([Step::Close(n), Step::Open(a.lhs(node))]),
                        Tag::Binary => work.extend([
                            Step::Close(n),
                            Step::Open(a.rhs(node)),
                            Step::Open(a.lhs(node)),
                        ]),
                        Tag::Ternary => {
                            let at = a.rhs(node);
                            work.extend([
                                Step::Close(n),
                                Step::Open(a.extra(at + 1)),
                                Step::Open(a.extra(at)),
                                Step::Open(a.lhs(node)),
                            ]);
                        }
                        // else: a string, a vector, an index or a bad
                        // expression is no bsource value.
                        _ => return None,
                    }
                }
                Step::Close(n) => {
                    let node = Node(n);
                    let op = |(s, compound): (String, bool)| match compound {
                        true => format!("({s})"),
                        false => s,
                    };
                    let s = match a.tag(node) {
                        Tag::Unary => {
                            let x = op(vals.pop().expect("an operand"));
                            match self.rd.t.text(a.main_token(node)) {
                                "-" => format!("-{x}"),
                                "+" => x,
                                "!" if ngspice => format!("{x}==0"),
                                _ => return None,
                            }
                        }
                        Tag::Binary => {
                            let y = op(vals.pop().expect("a right operand"));
                            let x = op(vals.pop().expect("a left operand"));
                            match self.rd.t.text(a.main_token(node)) {
                                o @ ("+" | "-" | "*" | "/" | "<" | "<=" | ">" | ">=" | "=="
                                | "!=" | "&&" | "||") => format!("{x}{o}{y}"),
                                "**" | "^" if ngspice => format!("pow(abs({x}),{y})"),
                                _ => return None,
                            }
                        }
                        Tag::Ternary => {
                            let b = op(vals.pop().expect("an else value"));
                            let t = op(vals.pop().expect("a then value"));
                            let c = op(vals.pop().expect("a condition"));
                            format!("{c} ? {t} : {b}")
                        }
                        _ => {
                            let n_args = a.list(a.rhs(node) + 1).len();
                            assert!(vals.len() >= n_args, "a call's arguments");
                            let args: Vec<String> =
                                vals.drain(vals.len() - n_args..).map(|v| v.0).collect();
                            let call = self.call(a.name(Name(a.lhs(node))), &args, &own)?;
                            vals.push((call, false));
                            continue;
                        }
                    };
                    vals.push((s, true));
                }
            }
        }
        assert_eq!(vals.len(), 1, "one value");
        vals.pop().map(|v| v.0)
    }

    /// Behavioural card `tok`'s statement, Spectre's `bsource` or VACASK's
    /// source line (`own` its name after the letter); `None` when it is
    /// refused.
    pub(super) fn bsource_card(
        &mut self,
        tok: u32,
        own: &str,
        current: bool,
        nodes: [&str; 2],
        expr: &str,
        value: u32,
    ) -> Option<String> {
        if super::checks::calls(expr).any(|c| c.eq_ignore_ascii_case("laplace")) {
            self.ask(Construct::Laplace, tok);
            return None;
        }
        if let Action::Refuse(_) = self.ask(Construct::BehaviouralSource, tok) {
            return None;
        }
        // The tree must be the whole value (`body_to_end` keeps
        // the text past a partial one).
        let whole = self.rd.last(value) == self.rd.line_last(tok);
        let Some(e) = self.bsource_expr(value).filter(|_| whole) else {
            self.refuse(Code::W0102, tok);
            return None;
        };
        // `[CMP]` p.859: `i = g * v`, `v = i * r`: a positive `i=`
        // flows from the first node through the source to the
        // second, as SPICE's; so does VACASK's (measured, P4-002).
        let key = if current { "i" } else { "v" };
        // The letter as written (`E1 … value=` is `e1`), as a
        // reference `i(E1)` spells it.
        let letter = self.rd.t.text(tok)[..1].to_ascii_lowercase();
        // VACASK: no master after the terminals is what makes the line a
        // behavioural source (`docs/dev-builtin-behavioral.md`, "Syntax").
        if self.b.named_rules == NamedRules::Vacask {
            let (p, n) = (q(nodes[0]), q(nodes[1]));
            return Some(format!("{letter}{} ({p} {n}) {key}={e}", q(own)));
        }
        Some(format!(
            "{letter}{} ({} {}) bsource {key}={e}",
            self.dev(own),
            self.node(nodes[0]),
            self.node(nodes[1])
        ))
    }

    /// The call of `name` (as written) on `args` (each a written operand)
    /// as Spectre's, `own` the deck's own functions (lowercase); `None`
    /// when Spectre has no function of its meaning. One arm per SPICE
    /// function: a rewrite (`limit` as `min(max)`, …) is one arm here.
    fn call(&self, name: &str, args: &[String], own: &[String]) -> Option<String> {
        let ngspice = self.rd.b.model_semantics == ModelSemantics::Ngspice;
        let f = name.to_ascii_lowercase();
        let spelled = match f.as_str() {
            f if SAME.contains(&f) => f.to_string(),
            "ln" => "log".to_string(),
            "log" if self.rd.b.card_rules != CardRules::Xyce => "log".to_string(),
            "pow" if ngspice && args.len() == 2 => {
                return Some(format!("pow(abs({}),{})", args[0], args[1]));
            }
            f if own.iter().any(|o| o == f) => name.to_string(),
            _ => return None,
        };
        Some(format!("{spelled}({})", args.join(",")))
    }

    /// `v(a)`, `v(a,b)` with its nodes, `i(dev)` as Spectre's
    /// `i("dev:0")` or VACASK's `i(dev)` of a voltage source; `None` for
    /// any other shape.
    fn probe(&self, fname: &str, args: &[u32]) -> Option<String> {
        let a = self.rd.ast;
        let word = |&k: &u32| {
            matches!(a.tag(Node(k)), Tag::Word | Tag::Ident | Tag::Num)
                .then(|| self.rd.text(k).trim().to_string())
        };
        let words: Vec<String> = args.iter().map(word).collect::<Option<_>>()?;
        let vacask = self.b.named_rules == NamedRules::Vacask;
        let node = |x: &str| if vacask { q(x) } else { self.node(x) };
        match (fname, words.as_slice()) {
            ("v", [x]) => Some(format!("v({})", node(x))),
            ("v", [x, y]) => Some(format!("v({},{})", node(x), node(y))),
            // VACASK reads a branch current only of an instance that has a
            // branch: `i(r1)` stops it ("Initial sparsity pattern creation
            // … failed"), `i(v1)` has ngspice's sign (both measured).
            // ponytail: a V source alone; L, E, H and a `v=` source carry a
            // branch too (`i(l1)` runs): add each with its run when needed.
            ("i", [d]) if vacask => d.starts_with(['v', 'V']).then(|| format!("i({})", q(d))),
            ("i", [d]) => Some(format!("i(\"{}:0\")", self.dev(d))),
            _ => None,
        }
    }
}
