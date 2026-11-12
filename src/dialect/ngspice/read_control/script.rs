//! A script statement's text → its commands → a tree of commands and loops. A block's lines
//! between `.control` and `.endc`, comment and blank lines dropped (`inp.c:742-760`), a `+` line
//! continuing the one before; a `*#` line is one command. `foreach` and `repeat` open a body
//! that `end` closes; `while`, `dowhile` and `if` also take an `end`, so they are parsed only to
//! keep the nesting and are flow for the planner.
//!
//! Cites: ngspice-45 `inp.c:742-812`, `control.c` (`cp_evloop`'s block kinds).
//!
//! Contents: `pub(super) struct Block`, `pub(super) struct Cmd`, `pub(super) enum Node`,
//! `pub(super) fn block(text: &str) -> Block`, `Cmd::word(&self) -> String`,
//! `Cmd::rest(&self) -> &str`, `Cmd::opaque_text(&self) -> String`

/// One script statement, parsed.
pub(super) struct Block {
    /// Its commands and loops, in order; `Err` when its nesting does not close.
    pub(super) nodes: Result<Vec<Node>, ()>,
}

/// One command: its text as written and its line with continuations joined.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Cmd {
    /// The lines as written, `\n` between (a `+` line kept as such).
    pub(super) text: String,
    /// The command with its continuations joined by one blank, trimmed.
    pub(super) line: String,
    /// A `*#` line (its own text is its opaque), not a block's command.
    pub(super) hash: bool,
}

/// A command or a loop.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Node {
    /// One command.
    Cmd(Cmd),
    /// `foreach var words … ; body ; end`.
    Foreach {
        /// The head.
        head: Cmd,
        /// The body.
        body: Vec<Node>,
    },
    /// `repeat [n] ; body ; end`.
    Repeat {
        /// The head.
        head: Cmd,
        /// The body.
        body: Vec<Node>,
    },
    /// `while`, `dowhile`, `if` … `end` (flow: the planner refuses it).
    Flow(Cmd),
}

/// The commands of one script statement (a block's raw text, or a `*#` line).
pub(super) fn block(text: &str) -> Block {
    let cmds = commands(text);
    Block { nodes: tree(&cmds) }
}

impl Cmd {
    /// The command word, lowercase (ngspice folds a control line, `inpcom.c:1775`).
    pub(super) fn word(&self) -> String {
        self.line
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
    }

    /// The line after the command word, trimmed.
    pub(super) fn rest(&self) -> &str {
        let line = self.line.trim_start();
        let end = line.find(char::is_whitespace).unwrap_or(line.len());
        line[end..].trim()
    }

    /// The text of this command's own opaque: a `*#` line as written, a block's command in its
    /// own `.control` pair (exact: blocks concatenate, `S3-CONTROL.md` O1).
    pub(super) fn opaque_text(&self) -> String {
        if self.hash {
            self.text.clone()
        } else {
            format!(".control\n{}\n.endc", self.text)
        }
    }
}

// ---- private ----

/// The commands of a statement's text.
fn commands(text: &str) -> Vec<Cmd> {
    let trimmed = text.trim_start();
    if let Some(hash) = trimmed.strip_prefix("*#") {
        let line = hash.trim().to_string();
        return vec![Cmd {
            text: text.trim_end().to_string(),
            line,
            hash: true,
        }];
    }
    let mut out: Vec<Cmd> = Vec::new();
    for raw in text.lines() {
        let l = strip_comment(raw.trim());
        let lower = l.to_ascii_lowercase();
        // a `*#` line in a block is its command (`inp.c:752-756` keeps it; `:777` strips `*#`)
        if let Some(cmd) = l.strip_prefix("*#") {
            out.push(Cmd {
                text: raw.trim_end().to_string(),
                line: cmd.trim().to_string(),
                hash: false,
            });
            continue;
        }
        if l.is_empty()
            || l.starts_with('*')
            || l.starts_with('#')
            || lower.starts_with(".control")
            || lower.starts_with(".endc")
        {
            continue;
        }
        if let Some(more) = l.strip_prefix('+')
            && let Some(last) = out.last_mut()
        {
            last.text.push('\n');
            last.text.push_str(raw.trim_end());
            last.line.push(' ');
            last.line.push_str(more.trim());
            continue;
        }
        out.push(Cmd {
            text: raw.trim_end().to_string(),
            line: l.to_string(),
            hash: false,
        });
    }
    out
}

/// `line` without its comment, as `inp_stripcomments_line` cuts a script line
/// (`inpcom.c:3500-3580`, `cs` set): from a `;` after the first character, a `$` followed by a
/// blank, or `//`, outside quotes; the blanks before it go too.
fn strip_comment(line: &str) -> &str {
    let b = line.as_bytes();
    let mut k = 0;
    // Bounded: each step moves `k` forward.
    while k < b.len() {
        let c = b[k];
        let next = b.get(k + 1).copied();
        if c == b'"' || c == b'\'' {
            k += 1;
            while k < b.len() && (b[k] != c || b[k - 1] == b'\\') {
                k += 1;
            }
            k += 1;
            continue;
        }
        if next == Some(b';') {
            return line[..k + 1].trim_end();
        }
        if (c == b'$' && next == Some(b' ')) || (c == b'/' && next == Some(b'/')) {
            return line[..k].trim_end();
        }
        k += 1;
    }
    line
}

/// The tree of `cmds`: loops and flow blocks closed by `end`; `Err` on an `end` with no opener
/// or an opener with no `end`.
fn tree(cmds: &[Cmd]) -> Result<Vec<Node>, ()> {
    // One frame per open block: its head and the nodes read so far.
    let mut stack: Vec<(Option<Cmd>, Vec<Node>)> = vec![(None, Vec::new())];
    for c in cmds {
        let w = c.word();
        match w.as_str() {
            "foreach" | "repeat" | "while" | "dowhile" | "if" => {
                stack.push((Some(c.clone()), Vec::new()));
            }
            "end" => {
                let (head, body) = stack.pop().ok_or(())?;
                let head = head.ok_or(())?;
                let node = match head.word().as_str() {
                    "foreach" => Node::Foreach { head, body },
                    "repeat" => Node::Repeat { head, body },
                    _ => Node::Flow(head),
                };
                stack.last_mut().ok_or(())?.1.push(node);
            }
            _ => stack.last_mut().ok_or(())?.1.push(Node::Cmd(c.clone())),
        }
    }
    if stack.len() != 1 {
        return Err(());
    }
    Ok(stack.pop().ok_or(())?.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_block_reads_its_commands_and_loops() {
        let b = block(".control\n* note\nforeach v 1k\n+ 2k\nalter r1 $v\nend\n\necho a\n.endc");
        let nodes = b.nodes.expect("closed");
        assert_eq!(nodes.len(), 2);
        let Node::Foreach { head, body } = &nodes[0] else {
            panic!("a foreach first: {nodes:?}")
        };
        assert_eq!(head.line, "foreach v 1k 2k");
        assert_eq!(head.text, "foreach v 1k\n+ 2k");
        assert_eq!(body.len(), 1);
        assert_eq!(
            nodes[1],
            Node::Cmd(Cmd {
                text: "echo a".into(),
                line: "echo a".into(),
                hash: false
            })
        );
    }

    #[test]
    fn a_script_lines_comment_is_cut_as_ngspice_cuts_it() {
        // inpcom.c:3519-3560 with `cs`: `$ `, `;`, `//`; `$v` and quoted text stay
        assert_eq!(
            strip_comment("meas tran t when v(2)=y $ not recognized!"),
            "meas tran t when v(2)=y"
        );
        assert_eq!(strip_comment("op ; a note"), "op");
        assert_eq!(strip_comment("alter r1 $v"), "alter r1 $v");
        assert_eq!(strip_comment("echo \"a $ b; c\" // x"), "echo \"a $ b; c\"");
        assert_eq!(strip_comment("tran 1u 5u"), "tran 1u 5u");
    }

    #[test]
    fn an_unclosed_or_stray_end_is_an_error() {
        assert!(block(".control\nforeach v 1\nop\n.endc").nodes.is_err());
        assert!(block(".control\nend\n.endc").nodes.is_err());
    }

    #[test]
    fn a_hash_line_is_one_command_whose_opaque_is_itself() {
        let nodes = block("*# pre_osdi a.osdi").nodes.expect("one");
        let Node::Cmd(c) = &nodes[0] else { panic!() };
        assert_eq!(c.word(), "pre_osdi");
        assert_eq!(c.rest(), "a.osdi");
        assert_eq!(c.opaque_text(), "*# pre_osdi a.osdi");
    }
}
