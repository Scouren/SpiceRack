//! Model cards: `.model name type (k=v …)` → one `Model` row: the family and polarity by
//! `families.rs` (the type word, its `level`, its version), the version, the parameters ngspice
//! sets (names lowercase, values as written), the bin base; or one `Opaque` with its code.
//!
//! ngspice's reading, rule by rule (ngspice-45 source):
//! - the type word picks the family: `INPdomodel`'s level switches for `npn pnp njf pjf nmf pmf
//!   nhfet phfet nmos pmos nsoi psoi` (`INPfindLev`: the first `level` of the line, `(int)(v +
//!   0.5)`; above 99 or below 0 ngspice runs level 1 instead: `InvalidInSource`), the diode's own
//!   `level` model parameter (1 or 3; any other runs neither family, `diotemp.c:210`: `Private`), one family
//!   for `r res c l sw csw urc ltra txl cpl`, `vdmos` with `pchan` (`inpcom.c`
//!   `inp_vdmos_model`), else `INPtypelook`: a code model, a built-in device name (`Unread`), or
//!   an OSDI module (`Module`);
//! - BSIM3, BSIM4 and HiSIM-HV pick their device by the first `version` of the line
//!   (`INPfindVer`); a version no prefix matches has no device (`InvalidInSource`); a silent card
//!   reads its device's default version (IR-CIRCUIT §4: "a reader whose card is silent writes its
//!   simulator's default version");
//! - `create_model` sets each name the device's tables have (a model parameter, or an instance
//!   parameter as the instances' default) and ignores the rest with "unrecognized parameter"
//!   (`level` and `m` silently): the reader drops them with a note, so the IR holds what ngspice
//!   sets; a repeated name is set again, the last value kept;
//! - the type word is itself read as a parameter, so `psoi`, `phfet` at level 1 and `pmf` at
//!   levels 5-6 set no polarity: the device runs its default n type (a note);
//! - a value that is no number gets braces (`inp_fix_param_values`): numparam reads it; a number
//!   is `INPevaluate`'s (`number::card`); one that reads `temper` is the B-source parser's (class
//!   `Temp`, R14); one that reads the circuit, a sequence or `true`/`false` is `Unread`;
//! - `INPmakeMod` keeps the first card of a name: a repeat in one scope is `Inert`;
//! - BSIM3, BSIM4, HiSIM2 and HiSIM-HV bin `base.<digits>` (`INPgetModBin`, `model_name_match`).
//!
//! The function validates the whole card before it pushes anything (law L1); the definition
//! (`DefKind::Model`, `Builder::model`) is the scope's, found by the bind step's nearest-scope
//! lookup as `inp_find_model` finds it. It never learns a target.
//!
//! Cites: ngspice-45 `inpdomod.c`, `inpfindl.c`, `inpfindv.c`, `inpgmod.c`, `inpmkmod.c`,
//! `inpcom.c` (`inp_vdmos_model`, `inp_fix_param_values`, `rem_mfg_from_models`,
//! `inp_find_model`), `inpeval.c` `INPevaluate`, `diotemp.c`; `rewrite/INV-semantics.md` 7.01, 7.02, 7.04, 7.11, 7.14,
//! 7.21; `rewrite/INV-constructs.md` F1–F8, F14, F16, F17; `rewrite/IR-CIRCUIT.md` §4.
//!
//! Contents: `pub(super) const FEATURES: ir::FeatureSet`, `pub(super) const CODES: &[Row]`,
//! `pub(super) fn model(lw: &mut Lower<'_, '_>, st: Stmt)`

use super::codes::{self, Row};
use super::cst;
use super::declare::Class;
use super::expr_read::{self, Tree};
use super::families::{self, Reading};
use super::number;
use super::read::{Lower, Stmt};
use super::tables::{self, Eval};
use crate::ir::{self, Code, Feature, Info, Kind, ModelFamily as F, OpaqueClass, Polarity};
use crate::syntax::ast::{NONE, Node, Tag};

/// What this area adds to `FEATURES`, the image of its reader: models, inside subcircuits too,
/// bins, with no device library, a string (a model's version), and the families ngspice has. A model in an `.if` branch (R33) joins as
/// `Placed(Branch, Model)` when the kernel's placement class lands (AMEND-A12 A15).
pub(super) const FEATURES: ir::FeatureSet = tables::feature_set(&[
    Feature::Item(ir::ItemKind::Model),
    Feature::Extra(ir::Extra::LocalModel),
    Feature::Circuit(ir::CircuitExtra::ModelBin),
    // ngspice's devices are built in: no model needs a `DeviceLibrary` (A12 C5)
    Feature::Circuit(ir::CircuitExtra::ModelWithoutLibrary),
    Feature::Expr(ir::ExprTag::Str),
    // the families ngspice-45 has a device for (`families::card`), a code model and a module
    Feature::Family(F::Resistor),
    Feature::Family(F::Capacitor),
    Feature::Family(F::Inductor),
    Feature::Family(F::Diode),
    Feature::Family(F::DiodeLevel3),
    Feature::Family(F::GummelPoon),
    Feature::Family(F::Vbic),
    Feature::Family(F::Hicum2),
    Feature::Family(F::Mos1),
    Feature::Family(F::Mos2),
    Feature::Family(F::Mos3),
    Feature::Family(F::Mos6),
    Feature::Family(F::Mos9),
    Feature::Family(F::Bsim1),
    Feature::Family(F::Bsim2),
    Feature::Family(F::Bsim3),
    Feature::Family(F::Bsim4),
    Feature::Family(F::B3SoiFd),
    Feature::Family(F::B3SoiDd),
    Feature::Family(F::B3SoiPd),
    Feature::Family(F::B4Soi),
    Feature::Family(F::Soi3),
    Feature::Family(F::Hisim2),
    Feature::Family(F::HisimHv),
    Feature::Family(F::Vdmos),
    Feature::Family(F::JfetShichman),
    Feature::Family(F::JfetParkerSkellern),
    Feature::Family(F::Mesfet),
    Feature::Family(F::Mesa),
    Feature::Family(F::Hfet1),
    Feature::Family(F::Hfet2),
    Feature::Family(F::SwitchHyst),
    Feature::Family(F::CurrentSwitchHyst),
    Feature::Family(F::Ltra),
    Feature::Family(F::Txl),
    Feature::Family(F::Cpl),
    Feature::Family(F::Urc),
    Feature::Family(F::CodeModel),
    Feature::Family(F::Module),
]);

/// This area's codes, `P1200–P1299, R1200–R1299`.
pub(super) const CODES: &[Row] = &[
    note(
        MALFORMED,
        "a model card with no name or type",
        "ngspice-45 inpdomod.c INPdomodel",
        "the card is kept as text (InvalidInSource).",
    ),
    note(
        NO_DEVICE,
        "a model card ngspice has no device for",
        "ngspice-45 inpdomod.c, inpfindl.c",
        "ngspice ignores the card or runs another level than written; it is kept as text (InvalidInSource).",
    ),
    note(
        UNREAD,
        "a model card form with no IR rule yet",
        "rewrite/S3-AREAS.md",
        "a level or version in braces, a flag word, a device name as type word, a sequence or a value that reads the circuit: kept as text (Unread).",
    ),
    note(
        CIDER,
        "a CIDER numerical device model",
        "ngspice-45 inpcom.c inp_cider_models",
        "ngspice's own numerical devices; kept as text (Private).",
    ),
    note(
        IGNORED,
        "a model parameter ngspice ignores",
        "ngspice-45 inpgmod.c create_model",
        "no parameter table of the device has the name (unrecognized parameter - ignored), or it is level or m, or a VDMOS annotation: dropped.",
    ),
    note(
        POLARITY,
        "a p-type word that sets no polarity",
        "ngspice-45 inpgmod.c create_model",
        "the type word is read as a parameter the device lacks; ngspice runs its default n type, and so does the IR row.",
    ),
    note(
        REPEATED,
        "a model parameter given twice",
        "ngspice-45 inpgmod.c create_model",
        "each value is set in turn; the last one is kept.",
    ),
    note(
        DIODE_LEVEL,
        "a diode level ngspice runs as neither family",
        "ngspice-45 diotemp.c:210, diosetup.c:301",
        "a level other than 1 and 3 scales the breakdown current by the area as level 3 does, without its geometry; kept as text (Private).",
    ),
    note(
        DUPLICATE,
        "a model defined twice in one scope",
        "ngspice-45 inpmkmod.c INPmakeMod",
        "ngspice keeps the first card of a name; this one changes no number and is kept as Inert text.",
    ),
];

/// A `.model` card with no name or no type word: `InvalidInSource`.
const MALFORMED: Code = Code::new(b'P', 1201);
/// A card ngspice has no device for: a level its switch lacks or above 99 or below 0 (it runs
/// level 1 instead), a BSIM3, BSIM4 or HiSIM-HV version no
/// prefix matches, `poly`, `ako:`: `InvalidInSource`.
const NO_DEVICE: Code = Code::new(b'P', 1202);
/// A card form the reader has no rule for yet: `Unread`.
const UNREAD: Code = Code::new(b'P', 1203);
/// A CIDER numerical device (`numd`, `nbjt`, `numos`): ngspice's alone, `Private`.
const CIDER: Code = Code::new(b'P', 1204);
/// A parameter ngspice ignores, dropped.
const IGNORED: Code = Code::new(b'P', 1205);
/// A p-type word that sets no polarity on its device: ngspice runs the n type.
const POLARITY: Code = Code::new(b'P', 1206);
/// A parameter given twice: ngspice sets the last value.
const REPEATED: Code = Code::new(b'P', 1207);
/// A diode level other than 1 and 3: ngspice runs it as neither family, `Private`.
const DIODE_LEVEL: Code = Code::new(b'P', 1208);
/// A model its scope defines already: ngspice keeps the first; this one is `Inert`.
const DUPLICATE: Code = Code::new(b'R', 1201);

/// A card's pieces: the name and type word as written, the `key = value` fields (keys
/// lowercase), the bare words after the type, and the text after the type, lowercase (what
/// `INPfindLev` and `INPfindVer` search).
struct Card {
    name: String,
    word: String,
    pairs: Vec<(String, Node)>,
    flags: Vec<String>,
    tail: String,
}

/// What a card is, before its parameters are read.
enum Head {
    Family(F, Polarity),
    Code(String),
    Module(String),
}

/// A refusal: the opaque's class and code.
type Refusal = (OpaqueClass, Code);

/// A parameter value, parsed and not yet pushed.
enum Value {
    /// A card number: its literal, and whether its digits were rounded.
    Number(ir::Literal, bool),
    /// A numparam expression.
    Expr(Tree),
    /// A quoted string, unquoted.
    Str(String),
}

/// A `.model` card → one `Model` row, or one `Opaque` with its code.
pub(super) fn model(lw: &mut Lower<'_, '_>, st: Stmt) {
    if let Err((class, code)) = read(lw, st) {
        lw.opaque(st, class, code);
    }
}

// ---- private ----

/// A note row.
const fn note(code: Code, title: &'static str, cite: &'static str, explain: &'static str) -> Row {
    (
        code,
        Info {
            title,
            cite,
            explain,
            kind: Kind::Note,
        },
    )
}

/// The card validated whole, then pushed; or why not.
fn read(lw: &mut Lower<'_, '_>, st: Stmt) -> Result<(), Refusal> {
    let card = parts(lw, st.node).ok_or((OpaqueClass::InvalidInSource, MALFORMED))?;
    let name = lw.names.fold(&card.name);
    // looked up, not interned: a refused card leaves no row, names included
    let found = lw.b.design().find_name(&name);
    if !found.is_none() {
        let prior = lw.b.design().lookup(st.scope, ir::DefKind::Model, found);
        if !prior.is_none() && lw.b.design().def(prior).scope == st.scope {
            return Err((OpaqueClass::Inert, DUPLICATE));
        }
    }
    let mut notes = Vec::new();
    let head = head(lw, &card, &mut notes)?;
    let (family, polarity) = match head {
        Head::Family(f, p) => (f, p),
        Head::Code(_) => (F::CodeModel, Polarity::None),
        Head::Module(_) => (F::Module, Polarity::None),
    };
    let version = match families::version(family) {
        Some((key, default)) => {
            let given = version(lw, &card, key, families::dispatches(family))?;
            Some(given.unwrap_or_else(|| default.to_string()))
        }
        None => None,
    };
    let device = match head {
        Head::Family(..) => {
            let given = version.as_deref().filter(|_| families::dispatches(family));
            let device = families::device(family, given);
            Some(device.ok_or((OpaqueClass::InvalidInSource, NO_DEVICE))?)
        }
        Head::Code(_) | Head::Module(_) => None,
    };
    if family == F::Txl {
        txl(lw, &card)?;
    }
    // the level is the family's, but MESA's (2-4) is a parameter of the device (mesa.c:108)
    let own_level = families::levelled(&card.word) && family != F::Mesa || card.word == "d";
    let key = families::version(family).map(|v| v.0);
    let kept = params(&card, own_level, key, device, &mut notes);
    let mut vals = Vec::new();
    for (key, node) in &kept {
        vals.push((*key, read_value(lw, st, *node)?));
    }
    // valid: push
    for code in notes {
        lw.note(st, code);
    }
    let id = lw.b.name(&name);
    let mut pairs = Vec::new();
    for (key, v) in &vals {
        pairs.push((*key, emit_value(lw, st, v)));
    }
    // the IR text parser's order: values, keys, bin base, version (one name table, L5)
    let params = lw.b.params(&pairs);
    let bin_of = bin(lw, &name, family);
    let none = ir::Ref::unresolved(ir::NameId::NONE);
    let (word, module) = match &head {
        Head::Code(w) => (lw.b.name(w), none),
        Head::Module(w) => (ir::NameId::NONE, ir::Ref::unresolved(lw.b.name(w))),
        Head::Family(..) => (ir::NameId::NONE, none),
    };
    let version = version.map_or(ir::ExprId::NONE, |v| lw.b.string(&v));
    let row = ir::Model {
        name: id,
        params,
        bin_of,
        ako: none,
        module,
        version,
        word,
        family,
        polarity,
    };
    let span = lw.span(st.node);
    lw.b.model(st.scope, row, span);
    Ok(())
}

/// The card's pieces, or `None` when it has no name or type word.
fn parts(lw: &Lower<'_, '_>, n: Node) -> Option<Card> {
    let fields = lw.ast.fields(n);
    let (&name, rest) = fields.split_first()?;
    let (&word, rest) = rest.split_first()?;
    let word_ok = |f: u32| lw.ast.tag(Node(f)) == Tag::Word;
    if !word_ok(name) || !word_ok(word) {
        return None;
    }
    let mut card = Card {
        name: lw.text(Node(name)),
        word: lw.text(Node(word)).to_ascii_lowercase(),
        pairs: Vec::new(),
        flags: Vec::new(),
        tail: String::new(),
    };
    for &f in rest {
        let f = Node(f);
        let text = lw.text(f);
        card.tail.push(' ');
        card.tail.push_str(&text.to_ascii_lowercase());
        match lw.ast.tag(f) {
            Tag::Assign if lw.ast.rhs(f) != NONE => {
                let key = lw.text(Node(lw.ast.lhs(f))).to_ascii_lowercase();
                card.pairs.push((key, Node(lw.ast.rhs(f))));
            }
            Tag::Word => card.flags.push(text.to_ascii_lowercase()),
            // a group, a stray value, an assignment with no value: no rule here yet
            _ => card.flags.push(String::new()),
        }
    }
    Some(card)
}

/// The family and polarity of the card's type word (and level), or the code model or module it
/// names; or why it has none.
fn head(lw: &Lower<'_, '_>, card: &Card, notes: &mut Vec<Code>) -> Result<Head, Refusal> {
    let word = card.word.as_str();
    if let Some(p) = vdmos(card)? {
        return Ok(Head::Family(F::Vdmos, p));
    }
    if word == "ako" || word.starts_with("ako:") {
        // PSPICE's derived model: the default mode reads `ako:x` as a type word it lacks (run:
        // "Unknown model type ako:q1 - ignored"); the PSPICE-compat flavour reads it later
        return Err((OpaqueClass::InvalidInSource, NO_DEVICE));
    }
    if !card.flags.is_empty() {
        // a flag: `pmos` after `nmos` would set the polarity again
        return Err(unread());
    }
    if families::levelled(word) {
        let level = level(lw, card, true)?.unwrap_or(1);
        if !(0..=99).contains(&level) {
            return Err((OpaqueClass::InvalidInSource, NO_DEVICE));
        }
        let f = match families::dispatch(word, level) {
            Reading::Family(f) => f,
            Reading::Invalid => return Err((OpaqueClass::InvalidInSource, NO_DEVICE)),
        };
        let mut p = families::polarity_word(word);
        let device = families::device(f, None).expect("a dispatched family has a device");
        if p == Polarity::P && !families::accepts(device, word) {
            notes.push(POLARITY);
            p = Polarity::N;
        }
        return Ok(Head::Family(f, p));
    }
    if word == "d" {
        // the diode's `level` is its own model parameter (`DIOlevel`, IF_INTEGER)
        return match level(lw, card, false)? {
            None | Some(1) => Ok(Head::Family(F::Diode, Polarity::None)),
            Some(3) => Ok(Head::Family(F::DiodeLevel3, Polarity::None)),
            // ngspice runs it, as neither family (diotemp.c:210): only ngspice computes it
            Some(_) => Err((OpaqueClass::Private, DIODE_LEVEL)),
        };
    }
    if let Some(f) = families::fixed(word) {
        return Ok(Head::Family(f, Polarity::None));
    }
    match word {
        "numd" | "nbjt" | "numos" => Err((OpaqueClass::Private, CIDER)),
        // `INPtypelook("POLY")`: no such device in this binary
        "poly" => Err((OpaqueClass::InvalidInSource, NO_DEVICE)),
        _ if families::device_name(word) => Err(unread()),
        _ if families::code_model(word) => Ok(Head::Code(word.to_string())),
        _ => Ok(Head::Module(word.to_string())),
    }
}

/// A VDMOS card's polarity (`inp_vdmos_model`: `pchan` anywhere, or `vdmosp`; `pchan` and
/// `nchan` dropped), `None` for any other type word.
fn vdmos(card: &Card) -> Result<Option<Polarity>, Refusal> {
    let p = match card.word.as_str() {
        "vdmos" | "vdmosn" => Polarity::N,
        "vdmosp" => Polarity::P,
        w if w.starts_with("vdmos") => return Err(unread()),
        _ => return Ok(None),
    };
    if card.flags.iter().any(|f| f != "pchan" && f != "nchan") {
        return Err(unread());
    }
    let pchan = card.flags.iter().any(|f| f == "pchan");
    Ok(Some(if pchan { Polarity::P } else { p }))
}

/// The card's level as ngspice reads it: `None` when it has none; `round` the `INPfindLev`
/// rule (`(int)(v + 0.5)`, the first "level" of the line), else the `IF_INTEGER` rule
/// (`floor(0.5 + v)`, the parameter). A level in braces, or a "level" `INPfindLev` would find
/// elsewhere, has no rule here.
fn level(lw: &Lower<'_, '_>, card: &Card, round: bool) -> Result<Option<i64>, Refusal> {
    let keys: Vec<Node> = keyed(card, "level");
    let found = card.tail.matches("level").count();
    if keys.len() > 1 || round && found != keys.len() {
        return Err(unread());
    }
    let Some(&node) = keys.first() else {
        return Ok(None);
    };
    if lw.ast.tag(node) != Tag::Word {
        return Err(unread());
    }
    let v = number_value(&lw.text(node)).ok_or_else(unread)?;
    // a decision on the level, not a number of the IR (R1 keeps the literal elsewhere)
    let level = if round {
        (v + 0.5) as i64
    } else {
        (0.5 + v).floor() as i64
    };
    Ok(Some(level))
}

/// The card's version text for keyword `key`, or `None` when it has none; one version given
/// as a word (`INPfindVer` reads the first "version" of the line when `dispatch`).
fn version(
    lw: &Lower<'_, '_>,
    card: &Card,
    key: &str,
    dispatch: bool,
) -> Result<Option<String>, Refusal> {
    let keys = keyed(card, key);
    if keys.len() > 1 || dispatch && card.tail.matches("version").count() != keys.len() {
        return Err(unread());
    }
    let Some(&node) = keys.first() else {
        return Ok(None);
    };
    if lw.ast.tag(node) != Tag::Word {
        return Err(unread());
    }
    Ok(Some(lw.text(node)))
}

/// A `txl` card is a TransLine only while R/L is outside (1.6e9, 1.6e10] (`INPdomodel` makes it
/// CplLines there); R and L must be numbers to tell.
fn txl(lw: &Lower<'_, '_>, card: &Card) -> Result<(), Refusal> {
    let get = |key: &str| -> Result<f64, Refusal> {
        match keyed(card, key).last() {
            None => Ok(0.0),
            Some(&n) if lw.ast.tag(n) == Tag::Word => number_value(&lw.text(n)).ok_or_else(unread),
            Some(_) => Err(unread()),
        }
    };
    let (r, l) = (get("r")?, get("l")?);
    // a decision on the device, as INPdomodel computes it
    let ratio = if l == 0.0 { r } else { r / l };
    if ratio > 1.6e9 && ratio <= 1.6e10 {
        return Err(unread());
    }
    Ok(())
}

/// The parameters ngspice sets, in card order, the last of a repeated name kept; the dropped
/// ones noted. `level` (when it picked the family: `own_level`) and the version keyword are the
/// row's fields; a code model's or module's names are all kept (no table here).
fn params<'c>(
    card: &'c Card,
    own_level: bool,
    version: Option<&str>,
    device: Option<&str>,
    notes: &mut Vec<Code>,
) -> Vec<(&'c str, Node)> {
    let mut kept: Vec<(&str, Node)> = Vec::new();
    for (key, node) in &card.pairs {
        if let Some(dev) = device {
            if key == "level" && own_level || Some(key.as_str()) == version {
                continue;
            }
            if !families::accepts(dev, key) {
                notes.push(IGNORED);
                continue;
            }
        }
        if let Some(k) = kept.iter().position(|(k, _)| k == key) {
            kept.remove(k);
            notes.push(REPEATED);
        }
        kept.push((key, *node));
    }
    kept
}

/// The bin base of `name` when ngspice bins `family` and the name is `base.<digits>`.
fn bin(lw: &mut Lower<'_, '_>, name: &str, family: F) -> ir::NameId {
    let base = name
        .rsplit_once('.')
        .filter(|(b, d)| !b.is_empty() && !d.is_empty() && d.bytes().all(|c| c.is_ascii_digit()));
    match base {
        Some((b, _)) if families::binnable(family) => {
            let b = lw.names.fold(b);
            lw.b.name(&b)
        }
        _ => ir::NameId::NONE,
    }
}

/// The value nodes of `key`, in card order.
fn keyed(card: &Card, key: &str) -> Vec<Node> {
    card.pairs
        .iter()
        .filter(|(k, _)| k == key)
        .map(|(_, n)| *n)
        .collect()
}

/// `Unread` with `P1203`.
fn unread() -> Refusal {
    (OpaqueClass::Unread, UNREAD)
}

/// The value at `node` of a card in `st`, or why it has no rule here.
fn read_value(lw: &Lower<'_, '_>, st: Stmt, node: Node) -> Result<Value, Refusal> {
    let text = lw.text(node);
    let expr = match lw.ast.tag(node) {
        Tag::Str => return Ok(Value::Str(cst::unquote(&text))),
        Tag::Word if numberish(&text) => {
            let s = number::card(&text).ok_or_else(unread)?;
            return Ok(Value::Number(s.literal, s.rounded));
        }
        Tag::Word if ["true", "false"].contains(&text.to_ascii_lowercase().as_str()) => {
            return Err(unread());
        }
        Tag::Word => text.as_str(),
        Tag::Delimited => unwrap(&text),
        _ => return Err(unread()),
    };
    // a value that reads `temper` is re-evaluated at each temperature by the B-source parser
    // (`inp_fix_temper_in_param`; run: vto={0.5+temper*1m} at .temp 27 and 127 gives
    // vto 0.527 and 0.627); one that reads the circuit has no rule here
    let eval = match lw.class(st.container, expr, &[]) {
        Class::Constant => Eval::Numparam,
        Class::Temp => Eval::Behavioural,
        Class::Circuit => return Err(unread()),
    };
    expr_read::parse_in(expr, eval)
        .map(Value::Expr)
        .map_err(|f| (f.class, f.code))
}

/// Pushes `v` and returns its root (a rounded number is noted).
fn emit_value(lw: &mut Lower<'_, '_>, st: Stmt, v: &Value) -> ir::ExprId {
    match v {
        Value::Number(lit, rounded) => {
            if *rounded {
                lw.note(st, codes::ROUNDED);
            }
            lw.b.literal(*lit)
        }
        Value::Expr(tree) => lw.expr(st, tree, st.scope),
        Value::Str(s) => lw.b.string(s),
    }
}

/// The double a card number token reads to (for a level or a txl decision), or `None` when it
/// is no number.
fn number_value(text: &str) -> Option<f64> {
    if !numberish(text) {
        return None;
    }
    number::card(text).map(|s| s.literal.value())
}

/// Whether `inp_fix_param_values` leaves `text` bare: a digit, a point, or a sign before a
/// digit or a point and a digit.
fn numberish(text: &str) -> bool {
    let b = text.as_bytes();
    let digit = |k: usize| b.get(k).is_some_and(u8::is_ascii_digit);
    match b.first() {
        Some(c) if c.is_ascii_digit() || *c == b'.' => true,
        Some(b'+' | b'-') => digit(1) || b.get(1) == Some(&b'.') && digit(2),
        _ => false,
    }
}

/// `text` without the one `{…}` or `'…'` that wraps all of it.
fn unwrap(text: &str) -> &str {
    let t = text.trim();
    let inner = t
        .strip_prefix('{')
        .and_then(|r| r.strip_suffix('}'))
        .or_else(|| t.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')));
    inner.unwrap_or(t)
}
