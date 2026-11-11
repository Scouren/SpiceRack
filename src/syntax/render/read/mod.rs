//! A file's [`Ast`] (and its kit) → the [`Deck`] plan the SPICE-family
//! writer spells: which statement is written in which block and in which of
//! the deck's places, as the old reader placed it, and the read-only views
//! (`Card`, `Val`, `Bench`, …) the writer asks of each statement. The plan
//! holds node ids and the few names the reader makes up (a hoisted
//! subcircuit's `parent__name`, the name map of instance masters); every
//! other word is spelled from the statement's tokens when it is written.
//!
//! **The old reader** is the PEG grammars and the lowering at `9d1d6bb`,
//! and this reader decides what each statement is by the same rules (`// quirk:`
//! marks each rule of the old reader it keeps), so the renderer's output is
//! the old pipeline's, byte for byte, which the golden gate checks
//! (TESTING.md, "Golden gate"). Phase 6 deletes the adapter and leaves these
//! rules here.
//!
//! This file is the spine: the plan's types and `read`; `dispatch` places
//! each statement (a SPICE card, a section's body, a card kept as text),
//! `blocks` walks the blocks on a heap stack, `text` joins tokens as the old
//! line stage did, `refused` sorts what the old grammar refused, `rejects`
//! leaves out a line its own simulator rejects (P0010, P0011), `values`,
//! `shapes`, `elements`, `devices` and `sources` read element cards,
//! `cards`, `testbench`, `outputs` and `options` dot cards (`model_cards`
//! the model card an instance uses), `control` a `.control` block's lines
//! (`unroll` its loops; `controlled` is E/F/G/H), `epochs` the run plan; `walk` steps a
//! block's branches for the writers, and drops them, on a heap stack.
//!
//! Cites: the old lowering
//! and its SPICE grammar's `card`, `Raw`, `Unparsed`.
//!
//! Contents:
//! - `pub(in crate::syntax::render) struct Rd`, `enum Kept`, `struct Deck`, `struct Block`, `enum Comp`,
//!   `enum Elem`, `enum Raw`, `struct Cond`, `struct Section`, `enum Func`
//! - `Block::elements(&self) -> Vec<Elem>`
//! - `fn read(ast: &Ast, t: &Tokens, d: Lang, name: &str, kit: Option<(&Kit, FileId)>) -> (Rd, Deck)`
//! - `Rd::text_behaviours`, `Rd::text_lang`, `Rd::role`, `Rd::kit_split`, `Rd::master_name`, `Rd::named`, `Rd::tag`, `Rd::main`, `Rd::fields`, `Rd::body`,
//!   `Rd::ttag` (`Rd::card`, `Rd::raw`, `Rd::unparsed`, `Rd::keep`, `Rd::section`: `dispatch.rs`)
//! - `fn unwrapped(t: &str) -> String`

use std::collections::{HashMap, HashSet};

use crate::syntax::Lang;
use crate::syntax::ast::{Ast, NONE, Node, Tag};
use crate::syntax::behaviour::{Behaviours, NamedRules};
use crate::syntax::diag::Diag;
use crate::syntax::render::spelling::Spelling;
use crate::syntax::resolve::{FileId, Kit, Role, Split};
use crate::syntax::token::{Tag as T, Tokens};

mod analyses;
mod blocks;
mod cards;
mod control;
mod controlled;
mod devices;
mod dispatch;
mod elements;
pub(in crate::syntax::render) mod epochs;
mod funcs;
mod model_cards;
mod named;
mod named_bench;
mod named_blocks;
mod named_control;
mod named_elements;
mod named_masters;
mod named_waves;
mod options;
mod outputs;
mod parallel;
mod refused;
mod rejects;
mod shapes;
mod sources;
mod sweeps;
mod tables;
mod testbench;
mod text;
mod unlift;
mod unroll;
mod values;
mod walk;
mod wave_rules;

pub(in crate::syntax::render) use cards::Model;
pub(in crate::syntax::render) use elements::Card;
pub(in crate::syntax::render) use model_cards::file_models;
pub(in crate::syntax::render) use named_bench::{spectre_refusal, vacask_refusal};
pub(in crate::syntax::render) use named_blocks::{Stats, Vary};
pub(in crate::syntax::render) use options::Opt;
pub use options::portable_option;
pub(in crate::syntax::render) use sources::{Num, Wave, WaveKind};
pub(in crate::syntax::render) use sweeps::{DcAxis, DcSweep, STEP_POINTS_MAX, step_points};
pub(in crate::syntax::render) use testbench::{Ana, BenchCard, Sweep};
pub(in crate::syntax::render) use unlift::{Lift, Step};
pub(in crate::syntax::render) use values::Val;
pub(in crate::syntax::render) use walk::Visit;

/// The reader: the file, its kit, and the deck-wide facts every statement's
/// view reads (the old lowering's `Cx`, less what it built).
pub(in crate::syntax::render) struct Rd<'a> {
    pub(in crate::syntax::render) ast: &'a Ast<'a>,
    pub(in crate::syntax::render) t: &'a Tokens<'a>,
    /// The language the file was read in (its text family: ESPice reads
    /// ngspice).
    pub(in crate::syntax::render) d: Lang,
    /// The file's language's behaviours (its family's: ESPice reads
    /// ngspice's).
    pub(in crate::syntax::render) b: &'static Behaviours,
    /// The deck's `.model` names, lowercased (the old lowering), each to
    /// its (last) card.
    models: HashMap<String, u32>,
    /// Spectre/VACASK: model kinds by name, and subcircuit names.
    kinds: HashMap<String, String>,
    subckts: Vec<String>,
    /// Spectre/VACASK: each instance by its scope (the `Subckt` node it
    /// sits in, `NONE` at the top) and name, and each one's scope: what
    /// a reference (`probe=`, `dev=`, `save dev:p`) names.
    insts: HashMap<(u32, String), u32>,
    scope_of: HashMap<u32, u32>,
    kit: Option<(&'a Kit, FileId)>,
    /// The name map: an `X` card's master as written, after the renames of
    /// the hoisted subcircuits it names (`parent__name`).
    masters: HashMap<u32, String>,
    /// The Monte Carlo read-back rule's fixups (`unlift`).
    unlifted: unlift::Unlift,
    /// A writer's `Lift` fixups, by token (`unlift::Fix`).
    fixes: HashMap<u32, unlift::Fix>,
    /// The target's spelling, once a writer names it ([`Rd::write_for`]):
    /// an expression's numbers are spelled in its suffixes.
    target: Option<&'static Spelling>,
    /// The `Num` nodes' tokens, sorted, once a writer names its target: the
    /// numbers it respells (a probe's `v(1a)` holds a name).
    nums: Vec<u32>,
    /// The top's Xyce `.global_param` assignments, read as `.param`s
    /// ([`Rd::global_param`]).
    global_params: HashSet<u32>,
    /// VACASK: the subcircuits the control block's one `elaborate` makes
    /// toplevel circuits, read as the top (`Rd::vacask_promoted`).
    promoted: HashSet<String>,
    /// Whether an LTspice capacitor's `x<n>` units are read as one scaled
    /// capacitor: once a writer names a target that writes a part's
    /// parasitics as elements ([`Rd::write_for`]).
    merge_units: bool,
    /// The top's DC sources read with LTspice's start-up ramp, once a
    /// writer whose target has no such start asks ([`Rd::ramp_startup`]).
    ramped: HashSet<u32>,
}

/// What the reader made of a file: every block and the deck-wide lists, in
/// the order the writer reads them.
#[derive(Default)]
pub(in crate::syntax::render) struct Deck {
    pub(in crate::syntax::render) top: Block,
    /// Every subcircuit at the top or nested in one (hoisted), in the order
    /// each closed.
    pub(in crate::syntax::render) defs: Vec<Block>,
    pub(in crate::syntax::render) sections: Vec<Section>,
    pub(in crate::syntax::render) globals: Vec<String>,
    pub(in crate::syntax::render) funcs: Vec<Func>,
    /// The testbench statements at the top, in read order (`.control`'s
    /// analyses among them); [`Rd::bench_cards`] reads each.
    pub(in crate::syntax::render) bench: Vec<Bench>,
    /// The reader's notes: what it read and leaves out.
    pub(in crate::syntax::render) notes: Vec<Diag>,
    /// The top's Verilog-A and OSDI loads not on a load card: a given
    /// ngspice deck's `.control` `pre_osdi` lines, then those given beside
    /// the text ([`super::Source::loads`]).
    pub(in crate::syntax::render) given_loads: Vec<String>,
    /// [`super::Source::numbers`].
    pub(in crate::syntax::render) numbers: super::Numbers,
}

/// A testbench item: a statement, and how it adds to the testbench.
#[derive(Clone, Copy, Debug)]
pub(in crate::syntax::render) enum Bench {
    /// A SPICE testbench card (`BenchCard`).
    Card(u32),
    /// A `.control` `alterparam` line a pass of a loop ran, and the value
    /// its variable gave it in that pass (`Rd::alterparam_as`).
    Set(u32, f64),
    /// A `.meas` line of a Spectre `simulator lang=spice` region.
    Meas(u32),
    /// Spectre/VACASK `save`.
    Save(u32),
    /// A Spectre analysis.
    Analysis(u32),
    /// Spectre `options`.
    Options(u32),
    /// Spectre `ic`/`nodeset`.
    NodeVals(u32),
    /// A VACASK `control` block.
    Control(u32),
}

/// One block's statements by place: a subcircuit, the top, a section's
/// body, a branch.
pub(in crate::syntax::render) struct Block {
    /// The name it is written under.
    pub(in crate::syntax::render) name: String,
    /// Its `Subckt` node; `NONE` for the top, a section, a branch.
    pub(in crate::syntax::render) node: u32,
    /// Its elements in order: typed cards and raw element lines.
    pub(in crate::syntax::render) comps: Vec<Comp>,
    /// Its `X` cards, each with the count of `comps` before it.
    pub(in crate::syntax::render) insts: Vec<(u32, u32)>,
    /// Its `.if` chains, each with the count of `comps` before it.
    pub(in crate::syntax::render) conds: Vec<(u32, Cond)>,
    /// Its `.model` cards: the statement, and the bin of a Spectre model
    /// group (`NONE`: none).
    pub(in crate::syntax::render) models: Vec<(u32, u32)>,
    /// Its `.param` assignments (at a deck's or section's top).
    pub(in crate::syntax::render) params: Vec<u32>,
    /// Its `.param` assignments inside a subcircuit.
    pub(in crate::syntax::render) locals: Vec<u32>,
    /// Its `.func` cards inside a subcircuit that read more than their
    /// arguments (a subcircuit parameter), or whose name another `.func`
    /// shares; any other moves to the deck's top ([`Rd::func`]).
    pub(in crate::syntax::render) funcs: Vec<Func>,
    /// Its `.include` cards.
    pub(in crate::syntax::render) includes: Vec<u32>,
    /// Its `.lib file section` cards.
    pub(in crate::syntax::render) libs: Vec<u32>,
    /// What it keeps as text, after its elements.
    pub(in crate::syntax::render) raw: Vec<Raw>,
    /// Its Verilog-A loads (Spectre `ahdl_include`, VACASK `load`).
    pub(in crate::syntax::render) loads: Vec<u32>,
    /// Its VACASK loads of VACASK's own linear R, C, L (`resistor.osdi`,
    /// `docs/dev-vacask.md`), whose instances are R, C, L cards: no load
    /// on another target, so VACASK's writer alone reads them.
    pub(in crate::syntax::render) own_loads: Vec<u32>,
    /// Its Spectre `statistics` blocks.
    pub(in crate::syntax::render) statistics: Vec<u32>,
}

/// One element of a block.
#[derive(Clone, Copy)]
pub(in crate::syntax::render) enum Comp {
    /// A card the writer spells (`Rd::card_view`).
    Card(u32),
    /// An element card kept as its line.
    Raw(u32),
}

/// One entry of a block's element order ([`Block::elements`]).
#[derive(Clone, Copy)]
pub(in crate::syntax::render) enum Elem {
    /// A typed card or a raw element line.
    Comp(Comp),
    /// An `X` card's node.
    Inst(u32),
    /// The index of the block's conditional.
    Cond(usize),
}

/// A statement kept as text.
#[derive(Clone, Copy)]
pub(in crate::syntax::render) enum Raw {
    /// Its logical line.
    Line(u32),
    /// A block kept whole, one line per line.
    Lines(u32),
    /// A block kept whole as one line holding its newlines.
    Joined(u32),
}

/// An `.if` chain: each branch's head (`If` or `Else`) and body.
pub(in crate::syntax::render) struct Cond {
    pub(in crate::syntax::render) branches: Vec<(u32, Block)>,
}

/// A `.lib name` … `.endl` section.
pub(in crate::syntax::render) struct Section {
    /// Its `Section` node.
    pub(in crate::syntax::render) node: u32,
    /// The Spectre `library` it sits in.
    pub(in crate::syntax::render) library: Option<String>,
    pub(in crate::syntax::render) body: Block,
    pub(in crate::syntax::render) defs: Vec<Block>,
    pub(in crate::syntax::render) funcs: Vec<Func>,
}

/// A function definition.
#[derive(Clone, Copy)]
pub(in crate::syntax::render) enum Func {
    /// `.param f(x)=…`: the assignment.
    Param(u32),
    /// `.func f(x) body`.
    Card(u32),
    /// Spectre `real f(real x) { return … }`.
    Real(u32),
}

/// What the old reader made of a card it could not lower.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(in crate::syntax::render) enum Kept {
    /// The grammar read it, the IR has no field: a raw line.
    Raw,
    /// The grammar refused it: an `Unparsed` line.
    Unparsed,
}

/// Where a card sits (the old lowering).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Scope {
    Top,
    Subckt,
    Section,
}

/// The deck state [`Rd::card`] appends to besides the block it writes into.
#[derive(Default)]
struct Acc {
    globals: Vec<String>,
    funcs: Vec<Func>,
    /// The top `.control` block's `pre_osdi` paths.
    control_loads: Vec<String>,
    defs: Vec<Block>,
    sections: Vec<Section>,
    bench: Vec<Bench>,
    notes: Vec<Diag>,
}

impl Default for Block {
    fn default() -> Block {
        Block {
            name: String::new(),
            node: NONE,
            comps: vec![],
            insts: vec![],
            conds: vec![],
            models: vec![],
            params: vec![],
            locals: vec![],
            funcs: vec![],
            includes: vec![],
            libs: vec![],
            raw: vec![],
            loads: vec![],
            own_loads: vec![],
            statistics: vec![],
        }
    }
}

/// Reads `ast` (the parse of `t`, in dialect `d`) into the plan the writer
/// spells, the top block named `name`; with `kit`, what the file alone
/// leaves open is the kit's answer.
///
/// # Panics
/// When `ast` is not `kit`'s tree of its file.
pub(in crate::syntax::render) fn read<'a>(
    ast: &'a Ast<'a>,
    t: &'a Tokens<'a>,
    d: Lang,
    name: &str,
    kit: Option<(&'a Kit, FileId)>,
) -> (Rd<'a>, Deck) {
    let d = d.behaviours().text_family;
    if let Some((k, f)) = kit {
        assert_eq!(ast.len(), k.node_count(f), "read: the kit file's tree");
    }
    let mut rd = Rd::new(ast, t, d, kit);
    let top_stmts = ast.fields(ast.root()).to_vec();
    if rd.named() {
        rd.promoted = rd.vacask_promoted(&top_stmts);
        (rd.kinds, rd.subckts) = rd.declarations(&top_stmts);
        rd.instances(&top_stmts);
    } else {
        rd.models = rd.model_names(&top_stmts);
    }
    let mut acc = Acc::default();
    let mut top = Block {
        name: name.into(),
        ..Block::default()
    };
    rd.cards(&top_stmts, &mut top, Scope::Top, &mut acc);
    if rd.b.named_rules == NamedRules::Vacask {
        // The old reader sorted the top's loads by path.
        top.loads.sort_by_key(|&n| rd.unquote(rd.fields(n)[0]));
    }
    let mut deck = Deck {
        top,
        defs: acc.defs,
        sections: acc.sections,
        globals: acc.globals,
        funcs: acc.funcs,
        bench: acc.bench,
        notes: acc.notes,
        given_loads: acc.control_loads,
        numbers: super::Numbers::Read,
    };
    rd.unlift(&mut deck);
    assert_eq!(deck.top.name, name, "the top under its name");
    assert_eq!(deck.top.node, NONE, "the top is no subcircuit");
    (rd, deck)
}

impl<'a> Rd<'a> {
    /// The behaviours of the language the text at token `tok` is in
    /// ([`Rd::text_lang`]).
    pub(in crate::syntax::render) fn text_behaviours(&self, tok: u32) -> &'static Behaviours {
        self.text_lang(tok).behaviours()
    }

    /// The dialect the text at token `tok` is in: a SPICE file's region's
    /// (`simulator lang=`), else the file's (a Spectre file's SPICE region
    /// is the file's, as the old reader kept it).
    pub(in crate::syntax::render) fn text_lang(&self, tok: u32) -> Lang {
        match self.named() {
            true => self.d,
            false => self.t.lexicon_at(tok).lang,
        }
    }

    /// Whether the file is Spectre or VACASK (statements by name), not SPICE.
    pub(in crate::syntax::render) fn named(&self) -> bool {
        self.b.named_rules != NamedRules::None
    }

    /// Node `n`'s tag.
    pub(in crate::syntax::render) fn tag(&self, n: u32) -> Tag {
        self.ast.tag(Node(n))
    }

    /// Node `n`'s main token: a statement's first, an item's anchor.
    pub(in crate::syntax::render) fn main(&self, n: u32) -> u32 {
        self.ast.main_token(Node(n))
    }

    /// Statement `n`'s items after its head, borrowed from the tree.
    pub(in crate::syntax::render) fn fields(&self, n: u32) -> &'a [u32] {
        self.ast.fields(Node(n))
    }

    /// Block `n`'s statements (empty for a flat statement), borrowed from
    /// the tree.
    pub(in crate::syntax::render) fn body(&self, n: u32) -> &'a [u32] {
        self.ast.body(Node(n))
    }

    /// Token `k`'s tag.
    ///
    /// # Panics
    /// When `k` is past the last token.
    pub(in crate::syntax::render) fn ttag(&self, k: u32) -> T {
        self.t.tags()[k as usize]
    }

    /// What the file is: the kit's answer, or the same rule's for a file
    /// read alone.
    pub(in crate::syntax::render) fn role(&self) -> Role {
        match self.kit {
            Some((kit, f)) => kit.file(f).role,
            None => crate::syntax::resolve::role(self.t, self.ast),
        }
    }

    /// The kit's corrected split of element `n`.
    pub(in crate::syntax::render) fn kit_split(&self, n: u32) -> Option<Split> {
        let (kit, f) = self.kit?;
        kit.split(f, Node(n))
    }

    /// `X` card `n`'s master, as the name map holds it.
    pub(in crate::syntax::render) fn master_name(&self, n: u32) -> String {
        match self.masters.get(&n) {
            Some(m) => m.clone(),
            None => self.name_of(self.ast.header(Node(n)).master),
        }
    }
}

impl Block {
    /// The block's elements in the order the old writers gave them: each
    /// instance, then each conditional, before the element it was read
    /// before (a stable sort by place, instances first).
    pub(in crate::syntax::render) fn elements(&self) -> Vec<Elem> {
        let n = self.comps.len();
        let at = |a: u32| (a as usize).min(n);
        let mut placed: Vec<(usize, Elem)> = self
            .insts
            .iter()
            .map(|&(a, i)| (at(a), Elem::Inst(i)))
            .collect();
        placed.extend(
            self.conds
                .iter()
                .enumerate()
                .map(|(k, &(a, _))| (at(a), Elem::Cond(k))),
        );
        placed.sort_by_key(|(k, e)| (*k, matches!(e, Elem::Cond(_))));
        let mut placed = placed.into_iter().peekable();
        let mut out = Vec::with_capacity(n + self.insts.len() + self.conds.len());
        for (k, &c) in self.comps.iter().enumerate() {
            while let Some((_, e)) = placed.next_if(|(p, _)| *p <= k) {
                out.push(e);
            }
            out.push(Elem::Comp(c));
        }
        out.extend(placed.map(|(_, e)| e));
        assert_eq!(
            out.len(),
            n + self.insts.len() + self.conds.len(),
            "every element once"
        );
        out
    }
}

/// A condition without the parentheses around it, blanks folded
/// (the old lowering's `unwrapped`).
pub(in crate::syntax::render) fn unwrapped(t: &str) -> String {
    let t = t.trim();
    let inner = t
        .strip_prefix('(')
        .and_then(|r| r.strip_suffix(')'))
        .filter(|r| {
            let mut depth = 0i32;
            r.chars().all(|c| {
                depth += i32::from(c == '(') - i32::from(c == ')');
                depth >= 0
            })
        })
        .unwrap_or(t);
    inner.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---- private ----

impl<'a> Rd<'a> {
    /// A reader of `ast` (the parse of `t`) in text family `d`, every
    /// deck-wide table empty: the one place a new table is added.
    fn new(ast: &'a Ast<'a>, t: &'a Tokens<'a>, d: Lang, kit: Option<(&'a Kit, FileId)>) -> Rd<'a> {
        Rd {
            ast,
            t,
            d,
            b: d.behaviours(),
            models: HashMap::new(),
            kinds: HashMap::new(),
            subckts: Vec::new(),
            insts: HashMap::new(),
            scope_of: HashMap::new(),
            kit,
            masters: HashMap::new(),
            unlifted: unlift::Unlift::default(),
            fixes: HashMap::new(),
            target: None,
            nums: Vec::new(),
            global_params: HashSet::new(),
            promoted: HashSet::new(),
            merge_units: false,
            ramped: HashSet::new(),
        }
    }
}
