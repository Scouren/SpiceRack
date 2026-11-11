//! An ngspice `.control` block's lines → the lines its script runs, in
//! run order ([`Rd::unroll`]). A loop (`foreach`, `repeat`, `while`,
//! `dowhile`) or an `if` whose words, count or condition is constant is
//! unrolled: its body's lines come once a pass, and an `alterparam`
//! whose value is a loop variable (`$v`) or a vector of the script
//! (`$&k`) comes with the value it set in that pass. The script is
//! followed the way ngspice runs it, a line at a time over a stack of
//! the open blocks, so a line a false `if` skips is never read.
//!
//! What is followed is what the script alone decides: a `foreach`
//! variable and its words, a `let` vector set from numbers and such
//! vectors (`+ - * /`, brackets), one comparison of two of those. A
//! vector is followed while no plot can hide it: the script sets it
//! before its first analysis (ngspice keeps it in the `const` plot,
//! which every later line sees and sets in place), no word of the deck
//! outside the block is its name (a node's vector would hide it), and
//! only `op`, `dc`, `tran` and `ac` run. A line that needs anything else
//! (a `while` over a run's result, a `$` variable a `set` made, `break`,
//! a `repeat` with no count) refuses the deck there (W0160), and so does
//! a script that runs more than [`UNROLL_MAX`] lines (W0161).
//!
//! Cites: ngspice-45 manual §13.6 (the control structures), §13.5.45
//! (`let`: "creates a vector in the current plot"), §13.3 (plots), §13.7
//! (`csnumprec`: a `$&` value has six digits; "the set command also
//! affects the behavior of the simulator via the options"), §13.2 (the
//! relational operators and their word forms); ngspice-45 source
//! `frontend/control.c` (`doblock`; a `repeat` count is read with its
//! block), `frontend/variable.c` (`span_var_expr`, `cp_varwl`: `%G`),
//! `frontend/options.c` (`cp_usrset`), `spicelib/analysis/cktsopt.c`
//! (`OPTtbl`); DESIGN-control-epochs §4, §8 D1 (ZERO-LEDGER P5-088). Run
//! on ngspice-45 (`research/impl/evidence/P5-088/RESULTS.txt`): every
//! form above, a node hiding a vector of its name, the six digits.
//!
//! Contents:
//! - `pub(super) const UNROLL_MAX: usize`
//! - `pub(super) struct Step`
//! - `Rd::unroll(&self, n: u32) -> Result<Vec<Step>, (Code, u32)>`

use super::Rd;
use crate::syntax::ast::Tag;
use crate::syntax::diag::Code;
use crate::syntax::render::models::fold;

/// The script lines one block may run, its loops unrolled (the design's
/// 4096 runs at four lines a pass): one bound for every loop, the one
/// that runs nothing too. Past it the deck is refused (W0161).
pub(super) const UNROLL_MAX: usize = 16_384;

/// One line the script ran.
pub(super) struct Step {
    /// The line's node.
    pub(super) line: u32,
    /// For an `alterparam` whose value held a variable: the value it set
    /// in this pass.
    pub(super) value: Option<f64>,
}

/// What a line is to the script's flow, by its command.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Flow {
    Foreach,
    Repeat,
    While,
    Dowhile,
    If,
    Else,
    End,
    /// Any other command: a line the script runs.
    Plain,
}

/// A block the script is in.
struct Frame {
    /// Its opening line's index.
    open: usize,
    /// `foreach`: the variable, then its words; empty for any other block.
    words: Vec<String>,
    /// `foreach`: the index of the next pass's word; `repeat`: the passes
    /// left after this one.
    at: usize,
}

/// The names the script itself gives a value, all lowercase (ngspice
/// lowercases a script's lines).
#[derive(Default)]
struct Vars {
    /// The `foreach` variables: name, and the word of the pass.
    words: Vec<(String, String)>,
    /// The `let` vectors that are followed: name, value.
    vectors: Vec<(String, f64)>,
    /// The `let` names a plot may hide ([`Rd::hidden`]): never followed.
    barred: Vec<String>,
    /// Whether an analysis has run: a vector first set after it is one of
    /// that analysis's plot, gone with the next (not followed).
    plotted: bool,
}

/// No line: a line with no partner in [`Rd::blocks`]' tables.
const NO_LINE: usize = usize::MAX;

/// The commands that neither make a plot nor a vector, lowercased, a
/// blank between two.
// ponytail: the common ones; any other command stops the vectors being
// followed (safe). Widen when a deck needs one.
const QUIET: &str = "alterparam echo plot print reset save wrdata write";

/// A comparison's test of its two sides.
type Test = fn(f64, f64) -> bool;

/// The comparisons a condition is read for: the sign or the word (ngspice
/// manual §13.2), the two-byte signs first, and the test.
const COMPARISONS: [(&str, Test); 12] = [
    ("<=", |a, b| a <= b),
    (">=", |a, b| a >= b),
    ("<>", |a, b| a != b),
    ("<", |a, b| a < b),
    (">", |a, b| a > b),
    ("=", |a, b| a == b),
    (" le ", |a, b| a <= b),
    (" ge ", |a, b| a >= b),
    (" ne ", |a, b| a != b),
    (" lt ", |a, b| a < b),
    (" gt ", |a, b| a > b),
    (" eq ", |a, b| a == b),
];

/// Every name ngspice-45 reads as a variable or an option, lowercased, a
/// blank between two: the literals of `cp_getvar`, `cp_vset`, `cp_remvar`
/// and `cp_usrset` in its source, `OPTtbl`, and the manual's §11.1 and
/// §13.7 lists (a `color<N>`, `hcopy…` or `auto_bridge…` one besides). A
/// `foreach` over one of these would set it.
const NGSPICE_VARS: &str = "\
    absdv abstol accept acct acfactortime acloadtime acsolvetime acsynctime actime addcontrol \
    addescape altshow appendwrite argc argv askquit autopartial autostop badmos3 batchmode \
    brief bypass chgtol controlswait convabsstep convlimit convstep copynodesets cpdebug \
    cptime cshunt cshunt_value csnumprec curplot curplotdate curplotname curplottitle debug \
    defad defas defl defm defw device diff_abstol diff_reltol diff_vntol digital_delay_type \
    diode_cj0 diode_rser display dyngmin echo editor enable_noisy_r epsmin equations \
    event_node_spacing factortime filetype fillinnz fourgridsize fournosave gmin gminfactor \
    gminsteps gnuplot_terminal gridsize gridstyle gshunt height helpboldfont helpbuttonfont \
    helpbuttonstyle helpinitxpos helpinitypos helpitalicfont helppath helpregfont \
    helptitlefont history histsubst indverbosity inputdir interactive interp itl1 itl2 itl3 \
    itl4 itl5 itl6 keep#branch keepopinfo klu klu_memgrow_factor level limpts limtim list \
    loadtime lprplot5 lprps lvlcod lvltim maxevtiter maxopalter maxord maxwarns measoutfile \
    method minbreak mingwpath modelcard modelline moremode nfreqs ngbehavior ngdebug nginfo \
    ng_nomodcheck noacct noasciiplotvalue no_auto_bridge_family no_auto_gnd nobjthack nobreak \
    noclobber node nodedamping noglob noinit nolegend nomod nonomatch noopac noopalter \
    noopiter nopadding nopage noparse noprintscale noquotesinoutput norefvalue nosavecurrents \
    nosighandling nosort no_spiceinit no_spinit nostepsizelimit nosubckt notrnoise nounits \
    numdgt numparams num_threads oldlimit opts originalnz oscompiled osdi_enabled pg_config \
    pivrel pivtol plainlet plainplot plainwrite plot_auto_spacing plothistory plots plotstyle \
    pointchars pointstyle polydegree polysteps printinfo probe_alli_given probe_alli_nox \
    probe_is_given program prompt ps_global_hash_table ps_global_tmodels ps_ports_and_pins \
    ps_scan_gates_optimize ps_tpz_delays ps_udevice_exit ps_udevice_msgs ps_use_mntymx \
    ps_with_inverters ps_with_tri_inverters ramptime rawfile rawfileprec rejected reldv \
    reltol remote_shell renumber reordertime rhost rndseed rprogram rsdiode rshunt \
    savecurrents scale scalm seed seedinfo sharedmode shellstatus silent_fileio sim_status \
    skywaterpdk soacheck solvetime sourcepath sparse spectrace specwindow specwindoworder \
    spicepath sqrnoise srcsteps statlocal strict_errorhandling strictnumparse subend \
    subinvoke substart svg_intopts svg_stropts synctime temp term ticchar ticlist ticmarks \
    time tnom totalnz totiter trancuriters tranfactortime traniter tranloadtime tranpoints \
    transolvetime transynctime trantime trantrunctime trtol trytocompact units unixcom vntol \
    warn wfont wfont_size width win_console wnflag wr_onespace wr_singlescale wr_vecnames \
    x11lineararcs xbrushwidth xfont xfont_size xgridwidth xmu xspice_enabled xtrtol";

impl Rd<'_> {
    /// The lines `.control` block `n`'s script runs, in run order: each
    /// plain line once a pass of the blocks around it, an `alterparam`
    /// that holds a variable with its value in that pass.
    ///
    /// # Errors
    /// The code and the first token of the line the script cannot be
    /// followed past: W0160 for a block that is not closed or a line that
    /// needs what the script alone does not decide, W0161 for a script
    /// that runs more than [`UNROLL_MAX`] lines.
    pub(super) fn unroll(&self, n: u32) -> Result<Vec<Step>, (Code, u32)> {
        assert_eq!(self.tag(n), Tag::Control, "a .control block");
        let lines = self.body(n);
        let unread = |i: usize| (Code::W0160, self.main(lines[i]));
        let hidden = |name: &str| self.hidden(name, n);
        let (mate, other) = self.blocks(lines).map_err(unread)?;
        let mut vars = Vars::default();
        let mut frames: Vec<Frame> = Vec::new();
        let mut steps = Vec::with_capacity(lines.len());
        let (mut pc, mut ran) = (0, 0);
        while pc < lines.len() {
            if ran == UNROLL_MAX {
                return Err((Code::W0161, self.main(lines[pc])));
            }
            ran += 1;
            let (word, rest) = self.script_line(lines[pc]);
            pc = match flow(&word) {
                Flow::Plain => {
                    // Flow that is not followed; a `quit` in a block ends
                    // the script before its last line.
                    let stops = matches!(word.as_str(), "break" | "continue" | "goto" | "label")
                        || (word == "quit" && !frames.is_empty());
                    if stops {
                        return Err(unread(pc));
                    }
                    let mut value = None;
                    if word == "alterparam" && rest.contains('$') {
                        // The name is the line's own (`Rd::alterparam_as`).
                        let named = rest.split_once('=').is_some_and(|(k, _)| !k.contains('$'));
                        let text = vars.subst(&rest).filter(|_| named);
                        let read = text.and_then(|t| self.alter_text(&t, self.main(lines[pc])));
                        value = Some(read.ok_or_else(|| unread(pc))?.1);
                    }
                    vars.plain(&word, &rest, &hidden);
                    steps.push(Step {
                        line: lines[pc],
                        value,
                    });
                    pc + 1
                }
                Flow::Foreach => {
                    let words = foreach(&rest).ok_or_else(|| unread(pc))?;
                    match words.get(1).cloned() {
                        None => mate[pc] + 1,
                        Some(first) => {
                            vars.set_word(&words[0], &first);
                            frames.push(Frame {
                                open: pc,
                                words,
                                at: 2,
                            });
                            pc + 1
                        }
                    }
                }
                Flow::Repeat => {
                    // ngspice reads the count with the block, before any
                    // of the script's lines ran: a plain count only.
                    let count: usize = rest.trim().parse().map_err(|_| unread(pc))?;
                    if count == 0 {
                        mate[pc] + 1
                    } else {
                        frames.push(Frame {
                            open: pc,
                            words: Vec::new(),
                            at: count - 1,
                        });
                        pc + 1
                    }
                }
                Flow::While | Flow::If => {
                    let yes = vars.cond(&rest).ok_or_else(|| unread(pc))?;
                    if !yes && other[pc] == NO_LINE {
                        mate[pc] + 1
                    } else {
                        frames.push(Frame {
                            open: pc,
                            words: Vec::new(),
                            at: 0,
                        });
                        if yes { pc + 1 } else { other[pc] + 1 }
                    }
                }
                Flow::Dowhile => {
                    frames.push(Frame {
                        open: pc,
                        words: Vec::new(),
                        at: 0,
                    });
                    pc + 1
                }
                // The true branch is done: on to the `end`, which closes
                // the `if`.
                Flow::Else => mate[mate[pc]],
                Flow::End => {
                    let f = frames.last_mut().expect("blocks: an `end` closes a block");
                    let open = f.open;
                    assert_eq!(open, mate[pc], "the innermost block");
                    let (word, rest) = self.script_line(lines[open]);
                    let again = match flow(&word) {
                        Flow::Foreach if f.at < f.words.len() => {
                            vars.set_word(&f.words[0], &f.words[f.at]);
                            f.at += 1;
                            true
                        }
                        Flow::Repeat if f.at > 0 => {
                            f.at -= 1;
                            true
                        }
                        Flow::While | Flow::Dowhile => {
                            vars.cond(&rest).ok_or_else(|| unread(open))?
                        }
                        // else: a loop that is done, or an `if`.
                        _ => false,
                    };
                    if again {
                        open + 1
                    } else {
                        frames.pop();
                        pc + 1
                    }
                }
            };
        }
        assert!(frames.is_empty(), "every block the script entered closed");
        assert!(steps.len() <= ran, "a step a line run at most");
        Ok(steps)
    }
}

// ---- private ----

impl Rd<'_> {
    /// Line `c`'s command and the text after it, both lowercased (ngspice
    /// lowercases a script's lines: `inpcom.c`).
    fn script_line(&self, c: u32) -> (String, String) {
        let head = self.main(c);
        let last = self.line_last(head);
        assert!(head <= last, "a line has a token");
        let rest = match head < last {
            true => self.span(head + 1, last).to_ascii_lowercase(),
            false => String::new(),
        };
        (self.t.text(head).to_ascii_lowercase(), rest)
    }

    /// Each line's partner in the script's blocks (`mate`): for a line
    /// that opens a block its `end`'s index, for an `end` or an `else`
    /// its opening line's; and each `if`'s `else` (`other`). [`NO_LINE`]
    /// where there is none.
    ///
    /// # Errors
    /// The index of an `end` or `else` with no block open, an `else`
    /// outside an `if` or after another, or a line whose block no `end`
    /// closes.
    fn blocks(&self, lines: &[u32]) -> Result<(Vec<usize>, Vec<usize>), usize> {
        let kinds: Vec<Flow> = lines
            .iter()
            .map(|&c| flow(&self.script_line(c).0))
            .collect();
        let mut mate = vec![NO_LINE; lines.len()];
        let mut other = vec![NO_LINE; lines.len()];
        let mut open: Vec<usize> = Vec::new();
        for (i, kind) in kinds.iter().enumerate() {
            match kind {
                Flow::Plain => {}
                Flow::Else => {
                    let &o = open.last().ok_or(i)?;
                    if kinds[o] != Flow::If || other[o] != NO_LINE {
                        return Err(i);
                    }
                    (other[o], mate[i]) = (i, o);
                }
                Flow::End => {
                    let o = open.pop().ok_or(i)?;
                    (mate[o], mate[i]) = (i, o);
                }
                Flow::Foreach | Flow::Repeat | Flow::While | Flow::Dowhile | Flow::If => {
                    open.push(i)
                }
            }
        }
        assert!(open.len() <= lines.len(), "an open block a line at most");
        match open.first() {
            Some(&o) => Err(o),
            None => Ok((mate, other)),
        }
    }

    /// Whether a plot of an `op`, `dc`, `tran` or `ac` run may hold a
    /// vector called `name`, which hides the script's own (run on
    /// ngspice-45: a node `k` beside `let k`): the plot's scale, or a
    /// node, which is a word of the deck outside `.control` block `n`.
    // ponytail: a node only an included file names is not seen; read the
    // includes' nodes here when a deck needs it.
    fn hidden(&self, name: &str, n: u32) -> bool {
        let first = self.main(n);
        let last = self.body(n).last().map_or(first, |&c| self.main(c));
        let last = self.line_last(last);
        assert!(first <= last, "the block's tokens");
        let outside = (0..first).chain(last + 1..self.t.len() as u32);
        matches!(name, "time" | "frequency")
            || outside
                .into_iter()
                .any(|k| self.t.text(k).eq_ignore_ascii_case(name))
    }
}

impl Vars {
    /// `foreach` variable `name` is `word` from here on.
    fn set_word(&mut self, name: &str, word: &str) {
        match self.words.iter_mut().find(|(k, _)| k == name) {
            Some(slot) => slot.1 = word.to_string(),
            None => self.words.push((name.to_string(), word.to_string())),
        }
    }

    /// What plain line `word rest` does to the variables: a `let` sets a
    /// vector, a `set` or `unset` may set a `foreach` variable it names
    /// ([`Vars::forget`]), a command of [`QUIET`] sets none, `op`, `dc`,
    /// `tran` and `ac` open a plot of the circuit's nodes, and any other
    /// command may make a plot or a vector of any name (`compose`,
    /// `meas`, `fft`, `noise` …), after which no vector is followed.
    /// `hidden` is [`Rd::hidden`].
    fn plain(&mut self, word: &str, rest: &str, hidden: &dyn Fn(&str) -> bool) {
        match word {
            "let" => {
                // A variable that is not followed leaves the `$` in the
                // text: the value does not fold, the name is not plain.
                let text = self.subst(rest);
                self.set(text.as_deref().unwrap_or(rest), hidden);
            }
            "set" | "unset" => self.forget(rest),
            w if QUIET.split(' ').any(|k| k == w) => {}
            "op" | "dc" | "tran" | "ac" => self.plotted = true,
            // else: any other command, see above.
            _ => {
                self.vectors.clear();
                self.plotted = true;
            }
        }
    }

    /// `let name = expr` (`text`: the line after `let`). A vector the
    /// script sets before its first analysis is one of ngspice's `const`
    /// plot, which every later line sees and sets in place (run on
    /// ngspice-45): it is followed while its value is a constant. One
    /// first set after an analysis, one a plot may hide (`hidden`) and one
    /// set to anything else is not.
    fn set(&mut self, text: &str, hidden: &dyn Fn(&str) -> bool) {
        // `let` alone lists the vectors.
        let Some((name, expr)) = text.split_once('=') else {
            return;
        };
        let name = name.trim();
        if !plain_name(name) {
            // An element, another plot's vector, a device's parameter:
            // any vector may be the one set.
            self.vectors.clear();
            return;
        }
        let at = self.vectors.iter().position(|(k, _)| k == name);
        match (at, self.value(expr)) {
            (Some(k), Some(v)) => self.vectors[k].1 = v,
            (Some(k), None) => {
                self.vectors.remove(k);
            }
            (None, Some(v)) if !self.plotted && !self.barred.iter().any(|b| b == name) => {
                match hidden(name) {
                    true => self.barred.push(name.to_string()),
                    false => self.vectors.push((name.to_string(), v)),
                }
            }
            (None, _) => {}
        }
        assert!(self.vectors.iter().all(|(_, v)| v.is_finite()), "finite");
    }

    /// A `set` or `unset` line (`rest`: the line after the command) may
    /// set each variable it names as a bare word (run on ngspice-45: `set
    /// noaskquit` in a `foreach noaskquit` loop, and `$noaskquit` is no
    /// longer the pass's word): a `foreach` variable among them is not
    /// followed until its loop sets it again.
    fn forget(&mut self, rest: &str) {
        let part = |c: char| c.is_ascii_alphanumeric() || c == '_' || c == '$';
        self.words
            .retain(|(k, _)| !rest.split(|c| !part(c)).any(|w| w == k));
    }

    /// `text` with each `$name` as its `foreach` word and each `$&name`
    /// as its vector's value the way ngspice writes one into a line (six
    /// significant digits, `%G`: run on ngspice-45). `None` for a
    /// variable that is neither, and for a form that is not read
    /// (`$name[…]`, `$?name`, `$#name` …).
    fn subst(&self, text: &str) -> Option<String> {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some((before, after)) = rest.split_once('$') {
            out.push_str(before);
            let body = after.strip_prefix('&');
            let name = body.unwrap_or(after);
            let len = name
                .bytes()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == b'_')
                .count();
            // ngspice's name runs on through these (`variable.c`,
            // `span_var_expr`).
            let runs_on = name[len..].starts_with(['-', '<', '#', '?', '@', '.', '(', '[', '&']);
            if len == 0 || runs_on {
                return None;
            }
            let (name, tail) = name.split_at(len);
            match body {
                Some(_) => {
                    let v = self.vectors.iter().find(|(k, _)| k == name)?.1;
                    out.push_str(&format!("{v:.5e}"));
                }
                None => out.push_str(&self.words.iter().find(|(k, _)| k == name)?.1),
            }
            rest = tail;
        }
        out.push_str(rest);
        assert!(!out.contains('$'), "subst: every variable read");
        Some(out)
    }

    /// `expr`'s value over the vectors that are followed: numbers (SPICE
    /// suffixes), those vectors, `+ - * /` and brackets. `None` for
    /// anything else (a call, a plot's vector, a result), and for a value
    /// that is not finite.
    fn value(&self, expr: &str) -> Option<f64> {
        let b = expr.as_bytes();
        let mut text = String::with_capacity(expr.len());
        let mut i = 0;
        while i < b.len() {
            let word = b[i..]
                .iter()
                .take_while(|c| c.is_ascii_alphanumeric() || **c == b'_' || **c == b'.')
                .count();
            if word == 0 {
                text.push(b[i] as char);
                i += 1;
                continue;
            }
            let w = &expr[i..i + word];
            if b[i].is_ascii_digit() || b[i] == b'.' {
                // A number with its exponent and suffix letters: as written.
                text.push_str(w);
            } else {
                // A name before `(` is a call.
                if b.get(i + word) == Some(&b'(') {
                    return None;
                }
                let v = self.vectors.iter().find(|(k, _)| k == w)?.1;
                text.push_str(&format!("({v:e})"));
            }
            i += word;
        }
        assert!(i == b.len(), "value: every byte read");
        fold(&text).filter(|v| v.is_finite())
    }

    /// `text` as a condition: one comparison of two values
    /// ([`COMPARISONS`]), or one value (true when not zero), each
    /// [`Vars::value`] after [`Vars::subst`].
    // ponytail: `&`, `|`, `!` and a comparison inside brackets are not
    // read (the sides do not fold, so the deck is refused); a parser
    // over ngspice's expression grammar when a deck needs one.
    fn cond(&self, text: &str) -> Option<bool> {
        let text = self.subst(text)?;
        let t = text.trim();
        // `(a < b)`: brackets around the whole condition.
        let inner = t.strip_prefix('(').and_then(|r| r.strip_suffix(')'));
        let t = inner.filter(|r| !r.contains(['(', ')'])).unwrap_or(t);
        for (sign, test) in COMPARISONS {
            if let Some((a, b)) = t.split_once(sign) {
                return Some(test(self.value(a)?, self.value(b)?));
            }
        }
        Some(self.value(t)? != 0.0)
    }
}

/// What a line whose command is `word` (lowercase) is to the flow.
fn flow(word: &str) -> Flow {
    match word {
        "foreach" => Flow::Foreach,
        "repeat" => Flow::Repeat,
        "while" => Flow::While,
        "dowhile" => Flow::Dowhile,
        "if" => Flow::If,
        "else" => Flow::Else,
        "end" => Flow::End,
        _ => Flow::Plain,
    }
}

/// Whether `s` is a name of letters, digits and `_` that starts with no
/// digit.
fn plain_name(s: &str) -> bool {
    let head = s.starts_with(|c: char| c.is_ascii_alphabetic() || c == '_');
    head && s.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}

/// A `foreach` line's variable, then its words (`rest`: the line after
/// the command). `None` when ngspice or its options read a variable of
/// that name ([`NGSPICE_VARS`]: the loop would set it), and when a word
/// is one the shell expands (a variable, a glob, a quote).
fn foreach(rest: &str) -> Option<Vec<String>> {
    let words: Vec<String> = rest.split_whitespace().map(str::to_string).collect();
    let name = words.first()?;
    let prefixed = ["color", "hcopy", "auto_bridge"];
    let reserved =
        NGSPICE_VARS.split(' ').any(|k| k == name) || prefixed.iter().any(|p| name.starts_with(p));
    let literal = |w: &String| {
        w.bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"_.+-".contains(&c))
    };
    (plain_name(name) && !reserved && words.iter().all(literal)).then_some(words)
}
