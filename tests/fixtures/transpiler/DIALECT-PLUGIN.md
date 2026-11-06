# Dialect plugins: a design

Status: design only (branch `dialect-plugin`, from `p7-integrate` b0952a7).
Nothing here is implemented. It lands after the in-flight conformance
branches that edit `src/syntax/render/` merge (section 6).

Decided by the user: a plugin is a Python class (a subclass of
`spicerack.Dialect`); it both reads its dialect's text into SpiceRack's one
AST and writes any deck in its dialect; it sees a narrow, stable surface and
none of the library's internals.

**The core choice.** A plugin is a built-in dialect with patched tables:
it names a `base` (one of the seven built-ins with a reader), and its data
replaces rows of the base's three tables (the `Lexicon`, the `Spelling`,
the capability column). It rides the base's tokenizer, parser, reader and
writer unchanged. A `Lang` handle (`u16`) names a reader-or-writer row; the
closed `Dialect` enum stays and means "which code path", so the 316 non-test
`Dialect::` sites do not change.

This is ESPice's pattern made public: ESPice already is "ngspice's reader,
its own `Spelling` row, its own capability column" (`Dialect::family`,
`render/spelling.rs` `ESPICE`, `capability.rs`'s `D::Espice` arms).

## 1. Caller code first

### A SPICE-family plugin: acme

Acme is ngspice with: `#` comment lines and `;` tail comments; `.subcircuit`
/ `.endsub` / `.transient` for `.subckt` / `.ends` / `.tran`; `x` as mega
(written in preference to `meg`); decimal number arithmetic; functions as
`.param f(x)='body'`; no Laplace sources (refused, its own code); charge
capacitors as `Q={expr}`; `reltol` spelled `rtol`; and a `.noise` card of its
own form that no table can say (a write hook).

```python
import spicerack as ps

class Acme(ps.Dialect):
    api = 1                       # the surface version this was written for
    name = "acme"
    base = "ngspice"              # reader, writer and every unlisted row

    # Reading and writing: each table reads one way and writes the other,
    # as the built-ins' letter and suffix tables do.
    comment_lines = ("*", "#")    # replaces the base's
    comment_tails = (";",)
    keywords = {                  # acme word -> base word; the first acme
        ".subcircuit": ".subckt", #   word per base word is the one written;
        ".endsub": ".ends",       #   the base's words still read
        ".transient": ".tran",
    }
    scales = (("x", 6), ("meg", 6), ("k", 3), ("m", -3), ("u", -6),
              ("n", -9), ("p", -12), ("f", -15), ("g", 9), ("t", 12))
    arithmetic = "decimal"        # how digits become a double, read and write
    func = "param"                # .param f(a)='body'
    options = {"reltol": "rtol"}  # SPICE option -> acme option, both ways

    # What acme can write, per construct (others: the base's answer).
    capabilities = {
        "laplace": ps.Refuse("ACME001"),
        "charge_capacitor": ps.Spell("charge_braced"),
    }
    codes = {                     # every code the plugin raises, and its message
        "ACME001": "acme has no Laplace sources",
        "ACME002": "acme's .noise takes one source",
    }

    # A hook: called once per deck with every .noise card the base wrote.
    def write_noise(self, cards):
        out = []
        for c in cards:           # ps.Card: head, words, params, text
            if len(c.words) != 3:
                out.append(ps.Refuse("ACME002"))
            else:
                out.append(".noise_acme out=%s src=%s %s" % tuple(c.words))
        return out

    hooks = {".noise": write_noise}

ps.register_dialect(Acme())       # once; validated here, frozen into Rust tables
```

The user code. Acme is netlist-only, as HSPICE is: `run(backend="acme")`
raises naming that no simulator runs it.

```python
text = open("amp.acme").read()
tb = ps.Testbench.from_source(text, dialect="acme")   # read acme
print(tb.netlist("ngspice"))                          # acme -> ngspice

tb2 = ps.Testbench.from_source(open("amp.cir").read(), dialect="ngspice")
print(tb2.netlist("acme"))                            # ngspice -> acme

bench = ps.Testbench(rc)                              # a Python deck
bench.add_transient(step_time=1e-9, end_time=1e-6)
print(bench.netlist("acme"))                          # Python -> acme

try:
    laplace_deck.netlist("acme")
except RuntimeError as e:                             # coded, positioned
    print(e)          # amp:12:1: W0201 acme:ACME001 acme has no Laplace sources
```

No new entry point: `from_source(text, dialect=)` and `netlist(name)`
already carry a dialect name; they accept a registered one.

### A named-family plugin: acmerf

Spectre with `*` comment lines too, `res`/`cap` as the masters for the
resistor and capacitor, `transient` for `tran`, its own option names, and no
`statistics` block.

```python
class AcmeRF(ps.Dialect):
    api = 1
    name = "acmerf"
    base = "spectre"
    comment_lines = ("//", "*")
    devices = {"res": "resistor", "cap": "capacitor"}  # acme master -> base master
    masters = {"transient": "tran"}                    # analysis/control words
    options = {"reltol": "rel_tol", "gmin": "g_min"}   # over the base's map
    capabilities = {
        "statistics": ps.Refuse("ARF001"),
    }
    codes = {"ARF001": "acmerf has no statistics block"}

ps.register_dialect(AcmeRF())
```

## 2. The surface

### Declarative data, evaluated once at `register_dialect`

`register_dialect(obj)` reads the class attributes once, hands them to Rust
as one `RowSpec`, and Rust validates and freezes them (section 3). After
that no Python runs during a read, and only hooks run during a write.
Every field is optional; a missing one inherits the base's row.

| Field | Table it patches | Read | Write | Vocabulary (checked at registration) |
|---|---|---|---|---|
| `name` | registry | `from_source(dialect=)` | `netlist(name)` | `[a-z][a-z0-9_-]*`, not a built-in name or alias |
| `base` | the code path | | | a built-in with a reader: ngspice, hspice, ltspice, xyce, spectre, vacask |
| `comment_lines`, `comment_tails` | `Lexicon` | yes | `Spelling.comment` = first of `comment_lines` | strings, no blank |
| `fold_case` | `Lexicon` | yes | | bool |
| `keywords` | `Lexicon.keywords` (a table before the base's) | acme → base tag | base word → first acme word | the base word is a keyword of the base lexicon |
| `scales` | `Lexicon.numbers.scales` | yes | first ASCII spelling per exponent | exponent on the writer's ladder (12..-15), one spelling per rung at least |
| `arithmetic` | `Numbers.arithmetic`, `Spelling.ngspice_numbers` | yes | yes | `"ngspice"`, `"decimal"` (one fact: the built-ins agree, `lexicon/*.rs`) |
| `devices` | `Lexicon.devices` (a table before the base's) | acme word → the base row's shape | primitive → first acme word | the base word is a row of the base's device table |
| `masters` | `Lexicon.masters` (named family) | yes | respelled after writing (§3.5) | the base word is a base master |
| `expr`, `path_quote`, `func`, `subckt_params` | `Spelling` | (the lexicon's quote rules are the base's) | yes | `func`: `func`, `param`, `real`, `none`; `expr` must be one the base lexicon reads |
| `options` | the option map | acme → SPICE option | SPICE → acme option | names only |
| `capabilities` | the capability column | | yes | construct names (stable list, §4); `ps.Native()`, `ps.Refuse(code)`, `ps.Spell(alt)`, `ps.Omit(code)` where `alt` is one the base **family's** writer implements |
| `codes` | the plugin's message table | | yes | `[A-Z]+[0-9]{3}` → message; every `Refuse`/`Omit` code is one of these |
| `hooks` | the hook table | | yes | base head words (`.noise`, `.four`, a SPICE element letter); methods of the instance |

What a plugin cannot do, by design: add a statement shape, a construct, an
element primitive, an analysis, an expression operator, or a lift rule. Each
is code in a reader or writer, and a plugin is data over code it does not
see. A field is added to this table when a second real plugin needs it
(factor at the second instance), never ahead.

### Hooks: requests, not callbacks

A table cannot say acme's `.noise_acme`. The Rust writer never calls
Python (the `syntax` owner has no I/O and builds for `wasm32`). Instead:

1. The base writer writes the deck, with the plugin's `Spelling` and
   cells, writing each hooked card in the base's form.
2. `render` returns the text and a **request table**: one row per hooked
   card, in output order: `{hook: u16, span: Range<u32>, anchor: u32}`.
3. The Python layer groups the rows by hook and calls each hook **once per
   deck** with the list of its cards, as `ps.Card` views (§4).
4. The hook returns one string or one `ps.Refuse` per card.
   `render::splice(written, answers)` replaces the spans back to front (one
   pass) and turns a refusal into a coded diagnostic; a wrong-length answer
   is a `ValueError` naming the hook.

So a hook runs per deck per hooked head, never per token or statement of
the hot loops; it holds the GIL only for its own call (the render releases
it). A deck with no hooked card makes no call. The Rust caller with no
plugin gets an empty request table and final text: the immediate twin.

Read hooks are not in this design: every read-side difference above is a
table, and a statement the tables cannot read stays what it is today
(`P0001`, a `Verbatim`). Open question 3.

## 3. Rust side

### 3.1 `Lang`: a handle beside the closed enum

```rust
/// A language a deck is read or written in: a built-in dialect or a
/// registered plugin. Built-ins are their `Dialect` index; plugins from
/// `Lang::FIRST_PLUGIN`.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Lang(u16);

impl Lang {
    pub const FIRST_PLUGIN: u16 = 16;
    pub fn base(self) -> Dialect;          // the code path: a built-in's own, a plugin's base
    pub fn name(self) -> &'static str;
    pub fn lexicon(self) -> Option<&'static Lexicon>;
    pub fn spelling(self) -> &'static Spelling;
    pub fn cells(self) -> &'static [Action; Construct::COUNT];
    pub fn from_name(s: &str) -> Option<Lang>;
}
impl From<Dialect> for Lang { .. }
```

Rejected: `Dialect::Plugin(u16)`. Every exhaustive `match` on a `Dialect`
would need an arm, and the 77 identity checks (`self.d == Dialect::Hspice`)
would have to decide what a plugin means at each; with `Lang`, the code
path's `Dialect` is `lang.base()` and every such site keeps its meaning.

Match sites (`rg` on `p7-integrate` b0952a7, `src/`):

| Sites | Count | Change |
|---|---|---|
| `Dialect::` mentions, all / outside tests | 363 / 316 | none, except the rows below |
| `capability.rs` per-construct `match target` blocks (arms naming `D::`) | 30 (110) | none: they become the const source of `CELLS` |
| `spelling.rs` | 27 | `spelling(Dialect)` stays for built-ins; `Lang::spelling` adds the registry |
| identity checks `== / != / matches!` on a dialect, `syntax/` | 77 (read 45, spice 16, named 15, other 1) | none: they read `base()` |
| other exhaustive `match` on a `Dialect` value (backend, python, lexicon, render) | 14 | `lexicon::of` only |
| `lexicon::of(` | 8 | → `Lang::lexicon` |
| `action(` outside `capability.rs` | 7 | → the writer's `cells` |
| `spelling(` outside `spelling.rs` | 5 | 2 of them → `Lang::spelling` (3 name ngspice/Xyce on purpose) |
| `read::read`, `Kit::load`, `Kit::dialect` | 2 + 5 + 4 | take / return `Lang` |
| reads of `Lexicon::dialect` | 5 | 3 keep the base; 2 (`parse` `Verbatim`, `read::text_dialect`) need the exact language |
| name parsing (`parse::<Dialect>`, `from_backend_name`) | 12 | the 4 in `python/` and `bin/spice2rack` → `Lang::from_name` |
| `render`, `render_text`, `render_kit` callers | 24 | none: they take `impl Into<Lang>` |

About 50 lines change outside the new files; the three likely changes have
radius zero (§3.6).

### 3.2 The registry

```rust
pub struct Row {
    pub name: &'static str,
    pub base: Dialect,
    pub lexicon: &'static Lexicon,   // the base's with the overrides, `lang` set
    pub spelling: &'static Spelling, // likewise
    pub cells: [Action; Construct::COUNT],
    pub heads: &'static [(&'static str, &'static str)],   // base word -> plugin word, written
    pub options: &'static [(&'static str, &'static str)], // SPICE -> plugin option
    pub hooks: &'static [&'static str],                   // base heads handed back
    pub codes: &'static [(&'static str, &'static str)],   // the plugin's code, message
}

pub const PLUGINS_MAX: usize = 64;
pub fn register(spec: RowSpec) -> Result<Lang, RegisterError>;
```

- **Where**: `src/syntax/lang.rs` (owner: `syntax`, beside `dialect.rs`).
  `RowSpec` is owned Rust data (strings, vectors), so a Rust test registers a
  plugin with no Python.
- **Storage**: `[OnceLock<&'static Row>; PLUGINS_MAX]` and an
  `AtomicU16` count; `register` appends under a `Mutex`. Rows are built,
  validated and leaked (`Box::leak`) before they are published, so a reader
  takes no lock and sees a whole row (`OnceLock`'s happens-before).
  `&'static Lexicon` is what the tokenizer already takes. Memory is bounded
  by `PLUGINS_MAX` rows; a row is never freed.
- **Idempotent**: registering a name again with an equal `RowSpec` returns
  its `Lang`; with a different one, `RegisterError::Redefined`. Fallible work
  (validation, building the tables) runs before the commit (the append).
- **Determinism**: a handle depends on registration order, and no output
  holds one (text, diagnostics and goldens carry names). Test: two
  registration orders, 1 thread and 8, the same bytes.
- **Lookup cost**: `Lang` → row once per `render` / `tokenize` call, never
  per token.

### 3.3 The tables taking a runtime row

- **Lexicon**: one new field `lang: Lang` (the exact language; `dialect`
  stays the base's code path). A plugin's lexicon is the base's `const` with
  the overridden slices replaced by leaked ones; `keywords` and `devices`
  become `[plugin_table, base_tables..]` ("the first table holding a word
  wins" already). The tokenizer and parser are unchanged: they take
  `&Lexicon` today.
- **Spelling**: unchanged shape, plus `options` (the named writer's
  `SPECTRE_OPTIONS` moves into Spectre's row; the SPICE rows' are empty).
  A plugin row is the base's with its fields replaced.
- **Capability**: `pub const CELLS: [[Action; Construct::COUNT]; 9]`,
  built by a `const fn` loop over the existing `action(target, c)`, so the
  exhaustive match stays the only source and a new construct still does not
  compile until every built-in answers. `Construct::ALL` and `COUNT` come
  from one list. A plugin's column is its base's with its cells replaced;
  `W::ask` reads `self.cells[c as usize]`.
- **Alt by family**: `Alt::family(self) -> Family` (SPICE or named): the
  writer that implements it. Registration refuses a `Spell` its base's
  writer does not write.

### 3.4 Zero cost on the hot paths

- Tokenizer and parser: the same `&'static Lexicon`; the struct gains a
  field the loops never read. Expected verdict IDENTICAL; P1 runs
  `asm_probe` on `tokenize::tokenize` before and after and cites it.
- `ask`: a const-fn `match` becomes a load from a 30-cell row resolved once
  per deck. Per construct occurrence, not per token. Expected EQUIVALENT;
  probed on `spice::W::ask`.
- Respell and hooks (§3.5) run only for a plugin target: the built-in path
  gains no branch.

### 3.5 What the base writer cannot spell: one post-pass

The SPICE writer writes card heads as literals (`".subckt"`, about 75 in
`render/spice/`), and the named writer writes masters and option names
likewise. Rather than thread a table through every literal, a plugin's
deck goes through one post-pass, `render/plugin.rs`, over the base
writer's text, at most once per deck:

- **Heads**: a statement-leading word in `Row::heads` is replaced (SPICE:
  a line's first word; named: the master after `name [(nodes)]`). The
  writer's output always starts a statement at a `Space::Line` token, and
  continuation (`+`, a trailing `\`) is the base's, which the pass reads
  through the base lexicon.
- **Hook spans**: a statement whose head is in `Row::hooks` becomes a
  request row (§2), its span the statement's lines.
- **Options**: SPICE family only in the pass (an `.options` card's keys);
  the named writer reads `Spelling::options` where it reads
  `SPECTRE_OPTIONS` today.

Risk: text a region keeps as written (`W0027` copies) is respelled too.
That is right for a plugin target (it is written in acme), and the fixpoint
oracle (§5) checks it.

### 3.6 Diagnostics

`Diag` is 8 bytes (asserted, `diag.rs:558`); `Code` is one byte, so a
`detail: u16` fits in the padding and the size assertion stays. Two new
catalogue rows in a new range:

| Range | Stage |
|---|---|
| W0201–W0300 | render: dialect plugins |

- `W0201 Refusal` "refused by a dialect plugin": `detail` is the index of
  the plugin's code in its row; the message is
  `<deck>:<line>:<col>: W0201 acme:ACME001 <the plugin's message>`.
- `W0202 Note`: `ps.Omit(code)`, written without the construct.
- A hook's refusal anchors to the cell's token where the writer asked,
  else to the first token of the source statement the span came from (the
  writer records it with the span).

The golden code line stays `codes: … W0201 …`; `golden diff` compares
codes, so a reworded plugin message is no difference.

The three likely changes, and their radius:

1. A new plugin field (expose `operators`): `RowSpec` and its validation,
   the Python conversion. Built-ins: zero.
2. A new `Construct`: the compiler lists every built-in arm (as today); a
   plugin inherits its base's cell: zero plugin edits. The Python name list
   test fails until the name is added (names compared, never counts).
3. A new built-in dialect: as today; plugins: zero.

## 4. Hidden, and what the plugin sees instead

Hidden: `Ast`, its `Tag`s and `Node` ids, `Tokens`, `Lexicon`'s Rust
enums (`Dollar`, `Backslash`, `Join`, `Quote`), the reader's plan (`Rd`,
`Deck`, `Card`, `Val`, `Bench`), `Out`, `Construct` and `Alt` as Rust
types, `Lang` numbers. The plugin sees only:

- **Names**: the base dialects, the construct names (snake_case of
  `Construct::ALL`: `laplace`, `charge_capacitor`, …), the alternatives
  per family (`charge_braced`, `vol_cur`, `bsource`, …), the base's keyword,
  device and master words. All are strings, listed by
  `ps.Dialect.vocabulary(base)` so a plugin author reads them, never source.
  Renaming one is a breaking change of `api`; the old name stays an alias
  for one release.
- **`ps.Card`** (frozen): `head: str`, `words: tuple[str, ...]`,
  `params: tuple[tuple[str, str], ...]` (in order), `text: str` (the card as
  the base wrote it, continuations joined). Built in Rust by reading the
  span with the plugin's lexicon: the plugin gets the base's text form, so
  its shape does not follow the AST.
- **`ps.Refuse(code)`, `ps.Spell(alt)`, `ps.Native()`, `ps.Omit(code)`**: plain value classes.
- **`api = 1`**: registration refuses a plugin written for another surface
  version.

## 5. Oracles

Each names what says the output is wrong, independent of the code.

1. **Self-clone (metamorphic, golden gate)**. For each built-in reader `B`
   (ngspice, HSPICE, LTspice, Xyce, Spectre, VACASK), a plugin
   `B-clone` with `base = B` and **every** table re-declared from a
   committed dump (`tests/fixtures/transpiler/plugin/<b>.json`, written
   once by `golden dump-row <b>` at a recorded commit: a golden with
   provenance), not inherited. Over the whole golden corpus, reading as
   `B-clone` and writing to every target, and reading as every input
   dialect and writing to `B-clone`, gives exactly the bytes and refusal
   codes the built-in `B` gives (`golden record` with a `--as plugin`
   flag, `golden diff` against the same run's built-in record; no xfail).
   Catches: a table field lost in conversion, a `Lang` that leaked into a
   `Dialect` site, a lookup that reads the base instead of the row.
   Run in Rust (P2) and again with the dump declared from Python (P4).
2. **Sibling clone (the inventory of what tables cannot say)**. ESPice as
   a plugin: `base = "ngspice"`, ESPice's `Spelling` and capability column.
   Its diff against the built-in ESPice is exactly the behaviour behind
   ESPice's identity checks (`spice/mod.rs:266`, `bench.rs:299`,
   `bench.rs:431`): a strict `plugin.xfail` naming each difference. When a
   check becomes a row field, its entries XPASS and fail until deleted.
3. **Round-trip fixpoint**. For the fixtures and a pinned 200-deck sample of
   `spice-bench`: `A1 = write(D, acme)`, `A2 = write(read(A1, acme), acme)`;
   `A1 == A2`, and every code of `write(D, acme)`'s refusals is in the
   expected name list. Same for `acmerf` over the Spectre fixtures.
4. **Simulator**. `write(read(write(D, acme), acme), ngspice)` run on
   ngspice in `.#testing` gives `D`'s ngspice results within the analysis's
   own tolerance (a sample of the `test_dialects.rs` decks). A missing
   ngspice fails.
5. **Refusals coded**. A deck with a Laplace source written to acme:
   `Err`, code list `[W0201]`, message `acme:ACME001` at the source's line.
   A hook's refusal likewise at its card. Each test is first run against a
   plugin with the cell set to `native`, and must go red there.
6. **Registration**. Every invalid field value (unknown base, a construct
   name, an `alt` of the other family, a keyword the base lacks, a suffix
   off the ladder, a duplicate code, a built-in's name) is a
   `ValueError` naming the field; registering twice is idempotent; a
   different redefinition errors; 65 plugins error at `PLUGINS_MAX`.
7. **Determinism**. Two registration orders × 1 and 8 threads, the same
   bytes; hooks called once per hook per deck, cards in output order.
8. **Vocabulary**. `ps.Dialect.vocabulary(b)` equals the Rust name lists
   (`Construct::ALL`, `Alt` by family) by name.

## 6. Plan

Ordered to land after the conformance branches that edit
`src/syntax/render/` merge; P1 is mechanical and conflicts with any of
them, so it starts from the merged `main`.

| Phase | Size | Files | Gate |
|---|---|---|---|
| P1 `Lang` | M | new `syntax/lang.rs` (handle, built-ins only); `dialect.rs`; `lexicon/mod.rs` (`lang` field, `of(Lang)`), each `lexicon/*.rs` (one line); `render/capability.rs` (`CELLS`, `Construct::ALL/COUNT`); `render/spelling.rs`; `render/mod.rs`, `read/mod.rs`, `spice/mod.rs`, `named/mod.rs` (`cells`, `base()`); `parse.rs` (`Verbatim`); `resolve/mod.rs` (`Kit` `Lang`) | golden gate byte-identical, no new xfail; `asm_probe` on `tokenize` and `ask` |
| P2 registry | M | `syntax/lang.rs` (`Row`, `RowSpec`, `register`, validation); `diag.rs` (`detail`, W0201–W0202, messages); `render/plugin.rs` (heads, options); `bin/golden` (`dump-row`, `--as plugin`); `tests/fixtures/transpiler/plugin/*.json` | oracles 1 (Rust), 2, 6, 7 |
| P3 acme | S | `tests/test_dialect_plugin.rs`; `tests/fixtures/transpiler/plugin/acme*` | oracles 3, 4, 5 |
| P4 Python | M | `src/python/dialect.rs` (`Dialect` base, `register_dialect`, value classes, `RowSpec` conversion); `deck.rs`, `store.rs`, `testbench.rs`, `subcircuit.rs` (names); `python/spicerack/__init__.py`; `docs/reference.md`, a `docs/dialects.md`; `skills/spicerack/` | oracle 1 from Python, 8; `test_docs.py` runs the acme example |
| P5 hooks | M | `render/plugin.rs` (spans, `splice`); `render/mod.rs` (`Written::requests`); `src/python/dialect.rs` (the driver, `ps.Card`) | oracle 5 (hooks), 7 (hooks) |
| P6 named heads | S | `render/plugin.rs` (masters after the header) | oracle 3 for `acmerf` |

Risks:

- **Identity checks**: a plugin behaves as its base wherever the code asks
  "is this HSPICE?", whatever its tables say. Oracle 2 lists that set for
  one sibling; each entry is a candidate row field, added when a plugin
  needs it.
- **Post-pass reads the writer's text**: it depends on the writer starting
  each statement on its own line. Oracle 1 cannot see it (no heads in a
  clone); oracle 3 can.
- **Construct names become a contract**: a rename in `capability.rs` is an
  `api` bump. Oracle 8 fails on any rename.
- **Leaked rows**: bounded by `PLUGINS_MAX`; a long-lived process that
  re-registers changed plugins (a notebook) gets `Redefined`, not a leak.

## Open questions for the user

1. Running a plugin dialect: netlist-only (as HSPICE), or a declared
   runner (a command line and the result format, one of the existing raw
   readers)?
2. `extra_line(line, dialect="acme")`: should a plugin get its own
   `simulator lang=acme` region in Python decks (a row in `NGSPICE_GIVEN`'s
   regions)? Not in this design.
3. Read hooks: needed? Every reading difference above is a table; a card no
   table reads stays `P0001`. A read hook would be a batch request over the
   `Verbatim` statements whose head the plugin claims, answered with base
   text and read again once.
4. Registration: explicit `ps.register_dialect(obj)` (this design), or
   automatic on subclassing (`__init_subclass__`, an import side effect)?
5. Should a plugin be able to base on another plugin? This design says no:
   `base` is a built-in, so a code path is always a built-in's.
