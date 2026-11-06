# Dialects as definitions: a design

Status: design only (branch `dialect-plugin`, from `p7-integrate` b0952a7).
Nothing here is implemented. It lands after the in-flight conformance
branches that edit `src/syntax/render/` merge (§9).

Decided by the user:

- A plugin is a Python class (a subclass of `spicerack.Dialect`, or of a
  built-in such as `spicerack.dialects.HSPICE`). It both reads its
  dialect's text into the one AST and writes any deck in its dialect, and
  it sees a narrow, stable surface.
- **Every** dialect (ngspice, ESPice, HSPICE, Xyce, LTspice, Spectre,
  VACASK, CDL, Verilog) is defined through the same interface. Built-ins
  are declared **in Rust** through a public definition API; the Python
  class calls the **same** API through a thin 1:1 PyO3 binding (same names,
  arguments, semantics). Every built-in is expressible with that public API
  alone: no private back door.
- The crate builds and runs with no Python (`--no-default-features`:
  wasm32, `golden`, Rust users). Built-ins need no interpreter.

## 1. The core choice

A dialect is a `DialectDef`: a builder whose calls fill the tables the
pipeline already reads (`Lexicon`, `Spelling`, a capability column), plus
named **behaviours** (closed enums, each selecting one existing code path
in a family's reader or writer, the way `Dollar::Hspice` selects the
tokenizer's `$` rule today). `register(def)` validates the resolved
definition once and freezes it into `&'static` rows behind a `Lang(u16)`
handle; the reader and writers read rows and behaviours and never ask "is
this HSPICE?". The built-ins are nine Rust functions returning a
`DialectDef`, registered on first use; a plugin is the same calls from
Python, usually `extends` a built-in and overriding a few rows.

What the API cannot say is a closed list (§6): the two family spines, four
writer layouts (CDL, Verilog, VACASK's, Xyce's options), the lift rules,
and about 15 behaviours. A definition selects among them and cannot add
one; that is the narrow surface. For what no row or behaviour says, a
plugin adds a **write hook**: a function of the cards of one head, called
once per deck (§6.3). Built-ins need zero hooks.

## 2. Caller code first

### One small dialect, twice

Acme is ngspice with `#` comment lines and `;` tail comments, `.subcircuit`
/ `.endsub` for `.subckt` / `.ends`, `x` as mega (written in preference to
`meg`), decimal arithmetic, functions as `.param f(x)='body'`, no Laplace
sources (its own refusal code), charge capacitors as `Q={expr}`, `reltol`
spelled `rtol`, and a `.noise` card of its own form (a hook).

Rust (a Rust user's crate, or a built-in's file):

```rust
use spicerack::syntax::def::*;

pub fn acme() -> DialectDef {
    DialectDef::new("acme")
        .extends(Lang::NGSPICE)
        .comment_line("#")
        .comment_tail(";")
        .keyword(".subcircuit", Keyword::Subckt)
        .keyword(".endsub", Keyword::Ends)
        .write_head(Keyword::Subckt, ".subcircuit")
        .write_head(Keyword::Ends, ".endsub")
        .scale("x", Scale::Exp(6))
        .arithmetic(Arithmetic::Decimal)
        .func(FuncForm::Param)
        .option("reltol", "rtol")
        .code("ACME001", Kind::Refusal, "acme has no Laplace sources")
        .code("ACME002", Kind::Refusal, "acme's .noise takes one source")
        .refuse(Construct::Laplace, "ACME001")
        .spell(Construct::ChargeCapacitor, Alt::ChargeBraced)
        .hook(".noise", write_noise)
}

fn write_noise(cards: &[Card], out: &mut Vec<Answer>) {
    for c in cards {
        out.push(match c.words {
            [o, s, n] => Answer::Text(format!(".noise_acme out={o} src={s} {n}")),
            _ => Answer::Refuse("ACME002"),
        });
    }
}

let acme: Lang = register(acme())?;
```

Python (the same calls in the same order; the same registered tables):

```python
import spicerack as ps
from spicerack.def_ import (Lang, Keyword, Scale, Arithmetic, FuncForm,
                            Construct, Alt, Kind, Answer)

class Acme(ps.Dialect):
    def define(self, d):
        return (d.extends(Lang.NGSPICE)
                 .comment_line("#")
                 .comment_tail(";")
                 .keyword(".subcircuit", Keyword.Subckt)
                 .keyword(".endsub", Keyword.Ends)
                 .write_head(Keyword.Subckt, ".subcircuit")
                 .write_head(Keyword.Ends, ".endsub")
                 .scale("x", Scale.Exp(6))
                 .arithmetic(Arithmetic.Decimal)
                 .func(FuncForm.Param)
                 .option("reltol", "rtol")
                 .code("ACME001", Kind.Refusal, "acme has no Laplace sources")
                 .code("ACME002", Kind.Refusal, "acme's .noise takes one source")
                 .refuse(Construct.Laplace, "ACME001")
                 .spell(Construct.ChargeCapacitor, Alt.ChargeBraced)
                 .hook(".noise", self.write_noise))

    def write_noise(self, cards):           # ps.Card: head, words, params, text
        return [Answer.Text(f".noise_acme out={c.words[0]} src={c.words[1]} {c.words[2]}")
                if len(c.words) == 3 else Answer.Refuse("ACME002")
                for c in cards]

acme = ps.register(Acme("acme"))  # DialectDef("acme"), define(), register: once
```

The enums are the Rust enums exported by name (`Keyword.Subckt` is
`Keyword::Subckt`; a data-carrying variant is a constructor,
`Scale.Exp(6)`); the binding has no vocabulary of its own. Oracle 3 (§8)
checks the two register byte-identical rows.

The user code. Acme is netlist-only, as HSPICE is (open question 1):

```python
tb = ps.Testbench.from_source(open("amp.acme").read(), dialect="acme")
print(tb.netlist("ngspice"))                         # acme -> ngspice
tb2 = ps.Testbench.from_source(open("amp.cir").read(), dialect="ngspice")
print(tb2.netlist("acme"))                           # ngspice -> acme
bench = ps.Testbench(rc)                             # a Python deck
bench.add_transient(step_time=1e-9, end_time=1e-6)
print(bench.netlist("acme"))                         # Python -> acme
try:
    laplace_deck.netlist("acme")
except RuntimeError as e:                            # coded, positioned
    print(e)   # amp:12:1: W0201 acme:ACME001 acme has no Laplace sources
```

`from_source(text, dialect=)` and `netlist(name)` already carry a dialect
name; they accept a registered one. Rust: `render_text(text, acme, "amp",
Lang::NGSPICE)`.

### Extending a built-in

```python
class MyHspice(ps.dialects.HSPICE):        # HSPICE.define is d.extends(Lang.HSPICE)
    def define(self, d):
        return (super().define(d)
                .comment_tail("//")
                .code("MY001", Kind.Refusal, "our HSPICE build has no .step")
                .refuse(Construct.Step, "MY001"))

ps.register(MyHspice("my-hspice"))
```

```rust
let my = DialectDef::new("my-hspice")
    .extends(Lang::HSPICE)
    .comment_tail("//")
    .code("MY001", Kind::Refusal, "our HSPICE build has no .step")
    .refuse(Construct::Step, "MY001");
```

Rust built-ins are extendable from Rust exactly as from Python; ESPice is
one (§3).

## 3. HSPICE, declared with the API

Every row of today's `lexicon/hspice.rs`, the `HSPICE` row of
`spelling.rs` and the `D::Hspice` cells of `capability.rs`, as calls. This
file (`src/syntax/dialects/hspice.rs`) replaces all three.

```rust
/// HSPICE (UG08 = User Guide 2008, CR10 = Command Reference 2010; the rule
/// ids L1..X2 are the old grammar's, `git show 9d1d6bb:grammar/hspice.peg`).
pub fn hspice() -> DialectDef {
    let quoted = Quoting { escapes: false, protects: true, across_breaks: true }; // T2, L6
    DialectDef::new("hspice")
        .family(Family::Spice)
        // Lines (L1-L9).
        .title(true)
        .fold_case(true)
        .comment_line("*").comment_line("$")
        .cont_skip("*").cont_skip("$")
        .dollar(Dollar::Hspice)                                    // L3
        .plus(true)                                                // L5
        .backslash(Backslash::Hspice)                              // L6
        .end(End { col1: false, then_blank: false, stops: true })  // L9
        .join(Join::None)
        // Words and quotes (T1, T2).
        .quote(Quote::Expr(quoted))
        .string(quoted)
        .string_expr(true)
        .ident_start("$").ident_continue("$.")
        .directive('.')
        .keywords(Keywords::Spice)                                 // the shared dot cards
        .keyword(".probe", Keyword::Probe)
        .lib_sections(true)
        // Operators (X2, UG08 pp.279-283, Table 19).
        .operator("**", Op::StarStar).operator("==", Op::EqualEqual)
        .operator("!=", Op::BangEqual).operator("<=", Op::LessEqual)
        .operator(">=", Op::GreaterEqual).operator("&&", Op::AmpAmp)
        .operator("||", Op::PipePipe)
        .infix(Op::PipePipe, 2).infix(Op::AmpAmp, 3)
        .infix(Op::EqualEqual, 4).infix(Op::BangEqual, 4).infix(Op::LessEqual, 4)
        .infix(Op::GreaterEqual, 4).infix(Op::Less, 4).infix(Op::Greater, 4)
        .infix(Op::Plus, 5).infix(Op::Minus, 5)
        .infix(Op::Star, 6).infix(Op::Slash, 6)
        .infix(Op::Caret, 7).infix(Op::StarStar, 7)
        .prefix(Op::Plus).prefix(Op::Minus).prefix_bp(6)
        .caret(Caret::Pow)
        .probe("v").probe("i")
        // Numbers (T3: T G MEG X K MIL M U N P F A).
        .exponent("eEdD")
        .scale("meg", Scale::Exp(6)).scale("x", Scale::Exp(6)).scale("mil", Scale::Mil)
        .scale("t", Scale::Exp(12)).scale("g", Scale::Exp(9)).scale("k", Scale::Exp(3))
        .scale("m", Scale::Exp(-3)).scale("u", Scale::Exp(-6)).scale("n", Scale::Exp(-9))
        .scale("p", Scale::Exp(-12)).scale("f", Scale::Exp(-15)).scale("a", Scale::Exp(-18))
        .tail(Tail::Spice { dot: true })
        .arithmetic(Arithmetic::Decimal)
        .devices(Devices::Spice)
        // Spelling.
        .comment("*")
        .path_quote("'")
        .expr("'", "'")
        .func(FuncForm::Param)
        .subckt_params(None)
        // Behaviours (§6.1): what the HSPICE identity checks did.
        .end_rule(EndRule::WhenRuns)                 // quirk: spice/mod.rs:169
        .value_quoting(ValueQuoting::Hspice)         // spice/cards.rs:216
        .sweep_form(SweepForm::Hspice)               // spice/bench.rs:338
        .out_names(OutNames::Bare)                   // read/outputs.rs:142
        .card_rules(CardRules::Hspice)               // read/testbench.rs:234
        // Capability: every construct answered (registration checks).
        .refuse(Construct::ForeignText, W0001)
        .refuse(Construct::CaretReread, W0002)
        .omit(Construct::NgBehavior, W0030)
        .native(Construct::StatisticalCall)
        .spell(Construct::VerilogALoad, Alt::Hdl)
        .refuse(Construct::OsdiLoad, W0004)
        .refuse(Construct::LibWholeFile, W0005)
        .native(Construct::LibSection)
        .native(Construct::RclModel)
        .spell(Construct::ChargeCapacitor, Alt::ChargeQuoted)
        .spell(Construct::BehaviouralSource, Alt::VolCur)
        .lift(Construct::Laplace, Rule::LaplaceTable)
        .refuse(Construct::RfPort, W0007)
        .refuse(Construct::PwlRepeatV, W0008)
        .refuse(Construct::PwlRepeatI, W0008)
        .refuse(Construct::NoiseWave, W0009)
        .native(Construct::Conditional)
        .refuse(Construct::BranchParam, W0014)
        .spell(Construct::Func, Alt::FuncParam)
        .refuse(Construct::Step, W0016)
        .native(Construct::Tf)
        .native(Construct::Sens)
        .refuse(Construct::SensAc, W0015)
        .refuse(Construct::SParameter, W0015)
        .refuse(Construct::PoleZero, W0015)
        .native(Construct::Print)
        .spell(Construct::Save, Alt::Probe)
        .native(Construct::UnmappedOption)
        .lift(Construct::Statistics, Rule::StatisticsDraws)
        .native(Construct::Measure)
}
```

About 100 lines, against today's 95-line lexicon const plus its spelling
row and 30 capability arms spread over two files. ESPice, the one built-in
that extends another:

```rust
pub fn espice() -> DialectDef {
    DialectDef::new("espice")
        .extends(Lang::NGSPICE)
        .control_block(ControlBlock::None)          // spice/mod.rs:266
        .noise_form(NoiseForm::Ngspice)             // spice/bench.rs:299, 431
        .omit(Construct::NgBehavior, W0030)
        .spell(Construct::VerilogALoad, Alt::Hdl)
        .refuse(Construct::OsdiLoad, W0004)
        .lift(Construct::Laplace, Rule::LaplaceTable)
        .native(Construct::PwlRepeatI)
        .refuse(Construct::NoiseWave, W0009)
}
```

(Behaviour names are read off today's code; P0 confirms each.)

## 4. The API

`syntax::def` (new, `pub`): `DialectDef`, `register`, `Lang`, `Card`,
`Answer` and the vocabulary enums. The Python module `spicerack.def_`
(PyO3, feature `python`) binds each 1:1: a method of the same name and
arguments; each Rust enum is a Python class of the same name whose
members are its variants.

| Group | Calls | Fills |
|---|---|---|
| Identity | `new(name)`, `extends(Lang)`, `family(Family)` | registry row; `Family::{Spice, Named}` picks the reader and writer spine |
| Lines | `title`, `fold_case`, `comment_line`, `comment_col1`, `blank_led_comment`, `cont_skip`, `comment_tail`, `block_comment`, `long_strings`, `dollar`, `plus`, `backslash`, `end`, `stop`, `join` | `Lexicon` |
| Words | `quote`, `string`, `string_expr`, `name_escape`, `ident_start`, `ident_continue`, `directive`, `head_words`, `head_after`, `keywords(Keywords)`, `keyword(word, Keyword)`, `lib_sections` | `Lexicon` |
| Expressions | `operator(text, Op)`, `infix(Op, bp)`, `right(Op)`, `prefix(Op)`, `prefix_bp`, `caret`, `probe`, `index`, `expr_braces` | `Lexicon.ops` |
| Numbers | `exponent`, `scale(text, Scale)`, `tail`, `arithmetic` | `Lexicon.numbers`, `Spelling.ngspice_numbers` (one fact) |
| Elements | `devices(Devices)`, `device(word, like)` (a row shaped as the base's row `like`), `master(word, Statement)`, `analysis(word, Analysis)`, `region(name, Lang)` | `Lexicon` |
| Spelling | `comment`, `path_quote`, `expr(open, close)`, `func`, `subckt_params`, `write_head(Keyword, text)`, `write_master(word, text)`, `option(spice, own)` | `Spelling`, the head, master and option maps |
| Capability | `native(c)`, `spell(c, Alt)`, `lift(c, Rule)`, `refuse(c, code)`, `omit(c, code)`, `copy(c, code)` | the capability column |
| Behaviours | one setter per behaviour (§6.1) | `Behaviours` |
| Codes | `code(id, Kind, message)` | the definition's message table (`W0201`/`W0202`) |
| Hooks | `hook(head, WriteHook)` | the hook table |
| Use | `register(def) -> Result<Lang, RegisterError>`, `Lang::from_name`, `Lang::name`, `Lang::dump` | |

About 70 calls. `Keywords::Spice`, `Devices::Spice` and the like name the
shared tables (`SPICE_KEYWORDS`, `device::SPICE`, …) so six definitions
do not repeat fifty rows; they are vocabulary values, as public as
`Keyword::Subckt`.

A cell's code is a catalogue `Code` (`W0001`, as the built-ins pass) or
one of the definition's own `code` ids. Both are open to every caller:
the catalogue is public and a plugin may refuse with `W0016` as HSPICE
does. Nothing in the API is reserved for built-ins.

## 5. Inheritance and registration

- **`extends(base)`** copies the registered base's resolved definition
  (every table, behaviour, cell, code and hook) into this one, `family`
  with it; later calls override. Any registered `Lang` may be a base.
- **Scalars and behaviours**: the last call wins.
- **Ordered tables** (keywords, scales, operators, devices, masters,
  options, regions): a row whose key the table holds replaces it in place;
  a new key goes **first**. An override is found first by the reader ("the
  first table holding a word wins", as today) and, where the writer reads a
  table backwards (letters, suffixes), it is what is written: acme's `x`
  for mega. `clear_<table>()` empties one; no per-row removal (factor at
  the second need).
- **Cells**: one per construct; the last call wins.
- **Validation, once, in `register`**, over the resolved definition, one
  validator for built-ins and plugins: the name is new, or an equal
  redefinition (idempotent: the same `Lang`), else
  `RegisterError::Redefined`; every construct has a cell; each `Alt` and
  `Rule` is one the family's writer implements (`Alt::family`,
  `Rule::family`); each behaviour value belongs to the family; each code is
  in the catalogue or the definition's table; each `region` names a
  registered language; the scales cover the writer's ladder (12..-15);
  a keyword's text fits the `directive` rule. An error names the call and
  its argument. All of it runs before the commit (the append).
- **Built-ins** register on first use of the registry in a fixed order
  (ngspice, espice, hspice, xyce, ltspice, spectre, vacask, cdl, verilog,
  then `ngspice-given`, the Python decks' reader whose regions name the
  others), so their handles are constants, `Lang::NGSPICE` = 0 to
  `Lang::NGSPICE_GIVEN` = 9.

## 6. What the tables cannot say

### 6.1 Behaviours, from today's identity checks

`syntax/` tests a dialect's identity 77 times outside `capability.rs` and
`spelling.rs`. By dialect named: VACASK 21, ngspice 17, Spectre 14, Xyce
12, LTspice 10, HSPICE 5, ESPice 3, CDL 2, Verilog 1 (by owner: reader 45,
SPICE writer 16, named writer 15, other 1). Each becomes one of:

1. **A row that already exists or is a table in disguise**:
   `caret_means_power` (`checks.rs`, twice) is the lexicon's `caret`;
   `values.rs:268, 310` are `numbers.arithmetic`; `cards.rs:211` (`incl`),
   `cards.rs:396` (`params`), `bsource.rs:165` (`log`),
   `named_elements.rs:213, 262, 263` (`pvcvs`, `njf`, `nmf`) are keyword,
   device and function-name rows. About 15 sites.
2. **A behaviour**: a closed enum named for what it does, set by the
   definition, read where the check was. Provisional (P0 reads each site):

   | Behaviour | Values | Sites today |
   |---|---|---|
   | `source_semantics` | `Ngspice`, `Other` (whose equations a read model or option means) | ~10 ngspice checks in `named/params.rs`, `named/cards.rs`, `bsource.rs`, `vacask_cards.rs`, `named/bench.rs:252`, `read/sources.rs`, `read/cards.rs` |
   | `control_block` | `Ngspice`, `None` | `spice/bench.rs:82`, `spice/mod.rs:95, 266` |
   | `noise_form`, `sweep_form` | `Ngspice`, `Hspice`, `Plain` | `spice/bench.rs:299, 338, 431` |
   | `end_rule` | `Always`, `WhenRuns` | `spice/mod.rs:169` |
   | `value_quoting` | `AsWritten`, `Hspice` | `spice/cards.rs:216` |
   | `model_params` | `Plain`, `Xyce` | `spice/mod.rs:367` |
   | `options_form` | `Plain`, `XycePackages`, `SpectreMap`, `VacaskMap` | `spice/bench.rs:142, 259`, `xyce_options.rs`, `read/options.rs:43`, `named/bench.rs:116` |
   | `card_rules` | `Spice`, `Hspice`, `Ltspice`, `Xyce` (the old reader's card quirks) | `read/testbench.rs:234-235, 436`, `read/cards.rs:155, 191, 321-322`, `read/shapes.rs:144`, `read/values.rs:254` |
   | `series_resistance` | `Plain`, `LtspiceRser` | `spice/mod.rs:419, 449` |
   | `out_names` | `Spice`, `Bare` | `read/outputs.rs:142` |
   | `named_rules` | `Spectre`, `Vacask` (instance parens, the `0` global, top-level forms, `spice/` loads, builtin masters) | ~16 checks in `read/named*.rs` |
   | `mc_readback` | `None`, `Spectre` | `read/unlift.rs:126, 385` |
   | `named_params` | `Spectre`, `Vacask` | `named/params.rs:51, 94, 168, 206, 222, 270` |
   | `path_sep` | `Slash`, … | `named/bench.rs:364` |
   | `writer` | `Cards`, `Cdl`, `Verilog` (SPICE); `Spectre`, `Vacask` (named) | `render/mod.rs:124`, `named/mod.rs:110`, `spice/mod.rs:105, 110` |

3. **Family code that stays code**: the two spines (`render/spice`,
   `render/named`, `render/read`), the layouts `writer` and `options_form`
   select (`cdl.rs` 227 lines, `verilog.rs` 177, `named/vacask*.rs`
   1,274, `xyce_options.rs` 85), the lift rules (`laplace.rs`,
   `named/lift.rs`, `draws.rs`), and the tokenizer rules `Dollar`,
   `Backslash`, `Tail`, `Arithmetic` already select. A definition picks
   among these by value; adding one is a change in the family's owner, as
   adding a `Construct` is today.

A behaviour is honest about its origin: `card_rules: Ltspice` means "the
old LTspice reader's card quirks", and a plugin based on LTspice gets
them. When one site needs a finer value, the enum gains a variant (factor
at the second instance), never a dialect test.

**Which built-in needs the most**: VACASK: 21 identity checks (12 in the
named reader, 9 in the named writer), its own writer layout (1,274 lines)
and the `VacaskCalls` lift. It needs **zero hooks**: every one is a row, a
behaviour value or family code. No built-in needs a hook; hooks are for
plugins.

### 6.2 Heads the writer writes as literals

The SPICE writer writes card heads as literals (`".subckt"`, about 75 in
`render/spice/`), the named writer its masters and option names likewise.
`write_head`, `write_master` and `option` act in one post-pass over a
plugin's written deck (`render/def_pass.rs`): a statement's leading word
(SPICE) or master (named, after `name [(nodes)]`) found in the map is
replaced. The built-ins declare none, so they skip the pass. Rejected:
threading a table through 75 literals, a larger diff for the same bytes.
Risk: text a region keeps as written is respelled too (right for a plugin
target; oracle 4 checks it).

### 6.3 Hooks: one signature, two callers

```rust
/// Writes the cards of one hooked head of a deck, as the definition's writer
/// wrote them, in output order: one answer per card, pushed onto `out`.
pub type WriteHook = fn(cards: &[Card<'_>], out: &mut Vec<Answer>);

pub struct Card<'a> {
    pub head: &'a str,
    pub words: &'a [&'a str],
    pub params: &'a [(&'a str, &'a str)],
    /// The whole card, continuations joined.
    pub text: &'a str,
}

pub enum Answer { Text(String), Refuse(&'static str) }
```

1. The writer writes every hooked card in its own form; the post-pass
   (§6.2) records a request row per hooked card, `{hook: u16, span:
   Range<u32>, anchor: u32}`, in output order.
2. A **Rust** hook is called in the post-pass, once per hooked head per
   deck, with all its cards; one `Vec<Answer>` is reused across hooks.
3. A **Python** hook cannot be called from `syntax` (no Python there, no
   I/O, wasm). `render` returns the request rows with the text; the binding
   calls each Python hook once per deck with its `ps.Card`s and hands the
   answers to `render::splice(written, answers)`, which replaces the spans
   back to front in one pass. A Rust caller without hooks gets no requests
   and final text: the immediate twin.
4. A wrong number of answers is a `ValueError` naming the hook; a refusal
   is `W0201` at `anchor`, the source statement the card came from.

A hook runs per deck per head, never per token or per statement of a hot
loop. Read hooks are not in this design (open question 3).

## 7. Rust side

### 7.1 `Lang`, and what happens to `Dialect`

`Lang(u16)` names a registered language: built-ins `0..10`, plugins after,
up to `LANGS_MAX = 64`. Inside `syntax`, every `Dialect` value becomes a
`Lang` and every identity check a row or behaviour read (§6.1), so
`syntax` no longer matches on a dialect. `Dialect` stays as the backend's
enum of the simulators it runs (`backend/` 46 sites, `python/` 33), mapped
to a `Lang` by name: running is the backend's owner, languages `syntax`'s.

Sites (`rg` on b0952a7, `src/`):

| Sites | Count | Becomes |
|---|---|---|
| `Dialect::` mentions, all / outside tests | 363 / 316 | the rows below; the backend's and Python's stay |
| `capability.rs` per-construct `match target` (arms naming `D::`) | 30 (110) | each built-in's capability calls; `action()` deleted |
| `spelling.rs` rows and `match` | 9 rows, 27 mentions | the definitions' spelling calls; `spelling()` deleted |
| identity checks in `syntax/` | 77 | ~15 row reads, ~55 behaviour reads, the `writer` dispatch |
| `lexicon::of` and its `match` | 8 calls | `Lang::lexicon` |
| uses of the lexicon consts (`lexicon::NGSPICE`, …) outside tests | 93 | `Lang::X.lexicon()`; the consts become definitions |
| `Lexicon::dialect` reads | 5 | `Lexicon::lang` |
| name parsing (`parse::<Dialect>`, `from_backend_name`) | 12 | `Lang::from_name` in `python/` and `bin/` |
| `render*` callers | 24 | none: `impl Into<Lang>` |

### 7.2 Registry

`[OnceLock<&'static Row>; LANGS_MAX]` and an `AtomicU16` count;
`register` appends under a `Mutex` after building and leaking the row
(`Box::leak`), so a reader takes no lock and sees a whole row. The
built-ins fill slots `0..10` in a `LazyLock` on first access. A row is
never freed; memory is bounded by `LANGS_MAX`. A plugin's handle depends
on registration order and appears in no output (text, diagnostics and
goldens carry names). `Lang::dump()` writes a row as canonical text
(every table, behaviour and cell): what oracles 2 and 3 compare.

### 7.3 A runtime builder, not `const` tables or spec files

- **The hot path is identical.** The tokenizer and parser take `&Lexicon`
  and get it at run time today (`lexicon::of` returns a reference chosen by
  a `match`), so no lexicon field is constant-folded into their loops now.
  A registered row is the same struct behind the same kind of reference,
  plus `lang: Lang`, which no loop reads. Expected `asm_probe` verdict on
  `tokenize::tokenize`: IDENTICAL; P1 cites the probe.
- **`ask` becomes a table load**: `W::ask` reads `cells[c as usize]` of
  the row resolved once per deck, where today it is a `const fn` match.
  Per construct occurrence, not per token. Expected EQUIVALENT, probed.
- **Behaviours** are a `Copy` struct of one-byte enums; a field load
  replaces a compare, per statement.
- **Registration cost**: the ten built-ins are about 1,500 calls, each a
  push or a store, once per process. No speed claim here: P1 measures it
  in a test that prints the time against a 1 ms budget.
- **Why not `const`**: a 1:1 binding needs per-row calls that push onto
  tables, which a `const fn` builder cannot do; a `const` form beside the
  builder is two APIs. **Why not spec files**: a third language and its
  parser for the same tables; the user chose Rust calls.
- **Totality moves** from compile time (the exhaustive `match`) to
  registration time: a new `Construct` compiles, and the pre-commit test
  "every built-in registers" fails naming each built-in without its cell.
  The price of the 1:1 API, stated.
- `--no-default-features` builds `syntax::def` and the built-ins; only
  `src/python/def_.rs` needs the `python` feature. wasm32 has `OnceLock` and
  `LazyLock`.

### 7.4 Diagnostics

`Diag` is 8 bytes (asserted, `diag.rs:558`) and `Code` one byte, so a
`detail: u16` fits the padding and the assertion stays. A new range:

| Range | Stage |
|---|---|
| W0201–W0300 | render: rules a dialect definition declares |

`W0201 Refusal` and `W0202 Note` carry `detail`, the index of the code in
the definition's table; the message reads `W0201 acme:ACME001 <message>`.
`golden diff` compares code lines, so a reworded plugin message is no
difference. Built-ins raise catalogue codes, as today: no golden changes.

## 8. Oracles

1. **The migration gate (metamorphic)**. A built-in moved onto its
   definition renders the golden corpus byte-identically: `golden record`,
   then `golden diff` against the previous phase's record, **IDENTICAL, no
   new `migration.xfail` line**, every input set and every target. The
   oracle is the previous code, which consulted no definition.
2. **API completeness (no back door)**. The built-in definition files
   (`src/syntax/dialects/*.rs`) are also compiled by an integration test
   (`tests/dialect_defs.rs`, an `include!` of each): an integration test is
   another crate and sees only `pub` items, so a definition using a private
   item fails to compile there. It registers each under a new name and
   checks `dump()` equals the built-in's.
3. **Python parity**. `tests/test_dialect_def.py` declares acme, and each
   built-in again with the same calls (written out, not generated), from
   Python; each `dump()` equals the Rust row's, byte for byte. A
   `MyHspice` subclass's dump equals the Rust `extends(Lang::HSPICE)` one.
4. **Round-trip fixpoint**. The fixtures and a pinned 200-deck
   `spice-bench` sample: `A1 = write(D, acme)`,
   `write(read(A1, acme), acme) == A1`; the refusal codes of
   `write(D, acme)` are the expected name list.
5. **Simulator**. `write(read(write(D, acme), acme), ngspice)` on ngspice
   in `.#testing` gives `D`'s results within each analysis's own tolerance
   (a sample of the `test_dialects.rs` decks). A missing ngspice fails.
6. **Refusals and hooks coded**. A Laplace deck written to acme: `Err`,
   code list `[W0201]`, `acme:ACME001` at the source line; a hook's refusal
   at its card. Each test is first run with the cell set to `native` and
   must go red there.
7. **Registration**. Each rule of §5 has a case naming the call;
   idempotent and conflicting redefinition; `LANGS_MAX + 1` errors; a
   definition missing one cell names it.
8. **Determinism**. Two plugin registration orders × 1 and 8 threads, the
   same bytes; hooks called once per head per deck, cards in output order.
9. **Names**. The Python enums' members equal the Rust variants (names
   compared, never counts).

## 9. Plan

Each phase starts from `main` after the conformance branches that edit
`src/syntax/render/` merge, and ends on oracle 1 IDENTICAL.

| Phase | Size | Files | Gate |
|---|---|---|---|
| P0 inventory | S | this doc: each of the 77 identity checks classified (row, behaviour, family code), with its site | review |
| P1 `def` and registry | M | new `syntax/def/{mod,build,validate,registry}.rs`; `Lexicon::lang`; `Lang` through `render`, `read`, `parse` (`Verbatim`), `resolve` (`Kit`); the built-ins still the old consts, wrapped by a **temporary** `DialectDef::from_tables` (the back door oracle 2 forbids; deleted in P9) | oracle 1; `asm_probe` on `tokenize` and `ask`; registration time |
| P2 behaviours | M | `syntax/def/behaviour.rs`; the 77 checks become row and behaviour reads, the old consts setting them | oracle 1 |
| P3 ngspice | M | `syntax/dialects/ngspice.rs`; `lexicon/ngspice.rs` keeps its tests | oracle 1 (6,050 decks and the PDKs), 2 |
| P4 ESPice | S | `dialects/espice.rs` (`extends(Lang::NGSPICE)`) | oracle 1, 2 |
| P5 HSPICE, Xyce, LTspice | M | `dialects/{hspice,xyce,ltspice}.rs`, one commit each | oracle 1, 2 |
| P6 Spectre, VACASK | M | `dialects/{spectre,vacask}.rs` (Spectre's `spice` region names HSPICE, so after P5) | oracle 1, 2 |
| P7 CDL, Verilog, `ngspice-given` | S | writer-only definitions (`writer(Cdl)`, `writer(Verilog)`, no lexicon rows); `ngspice-given` extends ngspice with its regions | oracle 1, 2 |
| P8 plugins | M | `render/def_pass.rs` (heads, masters, options, hook spans, Rust hooks), `W0201`/`W0202`; acme in Rust | oracles 4-8 |
| P9 cleanup | S | delete `from_tables`, `capability::action`, `spelling()` and its rows, `lexicon::of`, `Dialect` in `syntax` | oracles 1, 2 |
| P10 Python | M | `src/python/def_.rs` (the 1:1 binding, `ps.Dialect`, `ps.dialects.*`, `ps.register`, Python hooks via `splice`); `python/spicerack/`; `docs/dialects.md`, `docs/reference.md`; `skills/spicerack/` | oracles 3, 9; `test_docs.py` runs §2 |

**Migration order, and why.** ngspice first: every other SPICE definition
is compared with it, ESPice and `ngspice-given` extend it, the Python
decks are its text, and its corpus is the largest (6,050 decks and three
PDKs), so the strongest gate runs on the move everything else rests on.
ESPice second: the smallest delta (3 checks) and the first `extends`, which
proves inheritance before anything depends on it. Then HSPICE (5 checks),
LTspice (10), Xyce (12), in growing behaviour count. Spectre after HSPICE
(its region reads HSPICE), VACASK last of the readers (the most
behaviours, its own layout). CDL and Verilog last: writer-only, and the
least gated (`--skip verilog`).

**Expected deletions** (P9): `capability::action` and its 30 matches
(~190 lines; `Construct`, `Action`, `Alt`, `Rule` stay as vocabulary); the
nine `Spelling` consts and `spelling()` (~100); `lexicon::of`; `Dialect`'s
role in `syntax` (most of `dialect.rs`, ~90 lines; `ALIASES` moves to
`backend`); the 77 identity checks, replaced in place (net about zero).
The seven lexicon consts become definitions (their tests stay). Added:
`syntax/def` (~600 lines with the validator), `def_pass.rs` (~150),
`python/def_.rs` (~400, mostly one line per call).

Risks:

- **The behaviour names are a guess until P0**: some checks will split a
  behaviour or join one. P0 reads every site; the golden gate catches a
  wrong grouping.
- **Totality at registration, not compile time** (§7.3): mitigated by the
  pre-commit test, not removed.
- **P1 and P2 touch every render file**: they collide with any open render
  branch, so they land in a window with none open.
- **The enums become a public contract**: `Keyword`, `Construct`, `Alt`,
  `Rule` and the behaviours are API; a rename is a breaking change of
  `spicerack.def_`, kept as an alias for one release.
- **Leaked rows**: bounded by `LANGS_MAX`; a notebook re-registering a
  changed plugin gets `Redefined`, not a leak.

## Open questions for the user

1. Running a plugin dialect: netlist-only (as HSPICE), or a definition call
   naming a simulator command and one of the existing raw readers?
2. Should a plugin get a `simulator lang=<name>` region in Python decks
   (`extra_line(dialect="acme")`), i.e. a `region` row in `ngspice-given`?
3. Read hooks: every reading difference above is a row or behaviour, and a
   card no row reads stays `P0001`. Needed?
4. May a plugin extend another plugin? This design allows it (`extends`
   takes any registered `Lang`); the code path is still a built-in
   family's.
5. Behaviours named for their origin (`card_rules: Ltspice`), or split into
   finer, neutral ones before they become API (more enums, a longer P2)?
