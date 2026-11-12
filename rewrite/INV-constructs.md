# INV-constructs: what one dialect-neutral IR must represent

Source tree read: `SpiceRack-merge` (branch `batch-22`), read-only. Dossiers: `/tmp/claude-1000/spicerack-golden/research/`.

**Dialect keys.** ng ngspice, es ESPice (reads ngspice text; writer row of its own), hs HSPICE, xy Xyce, lt LTspice, sp Spectre, va VACASK. Writer-only targets: cdl (CDL, xschem layout), vlog (structural Verilog). Reader-only: py = `ngspice-given` (the Python API's decks: ngspice text + `.step` + `simulator lang=` regions), nx = `spicerack` (ngspice-given + the neutral expression grammar, decision D16).

**Path keys.** `cap` = `src/syntax/render/capability.rs`; `dev` = `src/syntax/device.rs`; `beh` = `src/syntax/behaviour.rs`; `lex` = `src/syntax/lexicon/mod.rs`; `tok` = `src/syntax/token.rs`; `ast` = `src/syntax/ast.rs`; `stmt` = `src/syntax/statement.rs`; `d/X` = `src/syntax/dialects/X.rs`; `rd/X` = `src/syntax/render/read/X.rs`; `ws/X` = `src/syntax/render/spice/X.rs`; `wn/X` = `src/syntax/render/named/X.rs`; `r/X` = `src/syntax/render/X.rs`; `res` = `src/syntax/resolve/mod.rs`; `diag` = `src/syntax/diag/rows_*.rs` (the code is the row). Dossiers: NG = `ngspice-source.md`, LH = `ltspice-hspice-source.md`, XY = `xyce-source.md`, VS = `vacask-spectre-source.md`, D-x = `DESIGN-x.md`, ZL = `ZERO-LEDGER.md`.

**Column "native in".** The dialects whose own text has the construct, per the reader rule or a `Native` capability cell. `?` = not established from what was read. A code in "seen at" such as `W0013` is the refusal a target lacking it gets.

Row ids (A1, B7, …) are stable handles for this file only. 537 rows in areas A–L, plus the 47-row capability matrix (M), the view types and the counts at the end. Within an area, rows come in source order: code first, then NG, LH, XY, VS, the design notes, the ledger; a later row may refine an earlier one (cross-read by construct name).

**Coverage (what was and was not read).**
- Read in full: `capability.rs`, `device.rs`, `statement.rs`, `kinds.rs`, `behaviour.rs`, `ast.rs`, `lexicon/mod.rs`, all eleven `dialects/*.rs`, `render/models.rs`, `render/spelling.rs`, the three params `.txt` tables (lines truncated), `resolve/mod.rs` (types), and in `render/read/`: `mod`, `elements`, `sources`, `testbench`, `options`, `sweeps`, `cards`, `named`, `named_bench`, `named_blocks`, `named_control`, `named_elements`, `named_masters`, `named_waves`, `wave_rules`, `controlled`, `devices`, `outputs`, `analyses`, `epochs`, `control`, `refused`, `dispatch`, `rejects`.
- Read in part: `token.rs` (the tag enum), `unlift.rs` (types, first 200 lines), `values.rs` (types), `unroll.rs`, `walk.rs`, `blocks.rs` (types); headers only for `funcs`, `shapes`, `text`, `parallel`, `preprocess`, `model_cards`, `tables`, `hb`, and for every writer module in `render/spice/` and `render/named/` (target-side spellings in this file come from those headers and from the capability cells).
- Diagnostics: code, kind and title of every row; full text only for `rows_lp.rs`.
- Dossiers: NG read in full; LH §1–§2 in full, §4–§7 with lines truncated; XY and VS by their construct and semantics columns (per-target decision cells not read); the eleven named design notes by their data sections (§2; pdk-binding §2, §3, §6). `DESIGN-custom-simulators` and `DESIGN-python-ast-builders` were not in the brief and were not read.
- ZERO-LEDGER: grepped only (dot-card words, `Construct::`/view names, W codes) against this file.

---

## A. File / deck structure

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| A1 | Title line | ng es hs xy lt sp va | Line 1 of a top-level file is the title whatever it holds; a library file has none (the kit reads it behind one empty title line so token numbers count it). | `ast::Tag::Title`; `lex::Lexicon::title`; `res::Form{Deck,Library}` |
| A2 | File role | all | Deck (holds an analysis, a control block or a top-level independent source) / Library (definitions only) / VerilogA source / Unread(NotFound or Refused). Role decides title, `.end`, control blocks, ground. | `res::Role`, `res::LoadError`; `r/role.rs` |
| A3 | `.end` card | ng es hs xy lt (sp, va: none) | ngspice drops it and reads on; HSPICE and LTspice stop reading (HSPICE: what follows is another run); Xyce only in column 1. Write rule: always vs only when the deck runs (HSPICE). | `ast::Tag::End`; `lex::End{col1,then_blank,stops}`; `beh::EndRule` |
| A4 | Stop byte | ng | A line whose first non-blank is `@` ends the file. | `d/ngspice` `.stop(Some(b'@'))` |
| A5 | Comment lines / tails | all, each its own set | ng: lines `* # $ ;` and stray punctuation (`= [ ] ? ( ) & % " ! : ,` FF, `//`), tails `;` `//`, `$` after blank; hs: `*` `$` lines, `$` hook; xy: `*` lines, blank-led lines, `;` tail that no quote protects; lt: `*` `;`; sp: `//` and `*` in column 1; va: `//` and `/* */`. Comments are not tokens today (live in token gaps): the IR decides whether to carry them. | `lex::Lexicon::{comment_lines,comment_col1,blank_led_comment,comment_tails,block_comment,dollar}`; `lex::Dollar{Text,AfterBlank,Hspice}` |
| A6 | Continuation | all | `+` first non-blank (ng es hs xy lt sp); trailing backslash: ngspice `\\` glue, HSPICE `\\` (no blank) and ` \` (keep blank), Spectre `\`, VACASK `\` tight before newline; open parens carry a Spectre statement; open brackets carry a VACASK one. Lexical only; the IR never sees it. | `lex::{Backslash,Join}`; `Lexicon::plus` |
| A7 | Case rule | fold: ng es hs xy lt; kept: sp va | Names compare case-blind in SPICE, as written in Spectre/VACASK. A SPICE-origin deck is lowercased for VACASK. The IR must keep the spelling and the compare rule. | `Lexicon::fold_case`; `wn/vacask.rs`; `r/names.rs` |
| A8 | `.include` | ng es hs xy lt | `.include`, `.inc`; Xyce `.incl` (rest of card is the path); LTspice rest-of-card path with blanks; path quotes `"…"`, `'…'` (HSPICE writes `'file'`). | `ast::Tag::Include`; `rd/cards.rs::include_path`; `r/spelling.rs::Spelling::path_quote` |
| A9 | `include "f"` | sp va | Quoted path; optional `section=name` makes it a library call. | `rd/named.rs` (`Tag::Include` + `section`) |
| A10 | Include scope | — | A file included inside a subcircuit defines local names (default) or global ones (Xyce): a deck read elsewhere loses the scope. | `beh::IncludeScope{Local,Global}`; W0046, W0128 |
| A11 | `.lib file section` call | ng es hs xy | Path + section name; file order among includes matters (a later `.param` wins on ngspice). | `ast::Tag::LibCall`; `rd/cards.rs::{lib_view,file_order}` |
| A12 | `.lib file` whole-file | lt (sp va native as plain include) | No section: the whole file; other SPICE targets spell it `.include`. | `cap::Construct::LibWholeFile`; `Alt::Include`; W0005, W0110 |
| A13 | `.lib name … .endl` section definition | ng es hs xy | Named section; body read as at the top; sections do not nest; LTspice has none (W0012). | `ast::Tag::Section`; `rd/mod.rs::Section{node,library,body,defs,funcs}`; `Construct::LibSection`; P0009 |
| A14 | `section name … endsection` | sp va | Same as A13 in named syntax. | `tok::KwSection/KwEndsection` |
| A15 | `library name … endlibrary` | sp | Wrapper around sections; a section remembers its library. | `ast::Tag::Library`; `rd/blocks.rs::Opened::Library` |
| A16 | `simulator lang=<x>` region | sp (lang=spice, read with the HSPICE lexicon); py (ngspice, spectre, vacask, hspice, ltspice, xyce, any registered plugin) | A run of statements in another dialect, read by that dialect's lexicon until the switch back; kept as that dialect's text except `.param` (expression values) and top `.meas` in a Spectre file, and Spectre analyses in a py deck. | `ast::Tag::Region`; `tok::{LangSpice,LangSpectre}`; `Lexicon::regions`, `regions_registered`; `rd/named.rs::region`; `Alt::SpiceRegion` |
| A17 | Verilog-A source load | sp `ahdl_include "f.va"`; hs es `.hdl "f"`; va `load "f.osdi"` (compiled) | One fact "load module file F", spelled per target; Xyce and LTspice have none (W0004); loads in a branch are `BranchParam`. | `Construct::VerilogALoad`; `beh::LoadForm{Ngspice,Espice,Hspice,Xyce,Ltspice,None}`; `Alt::{Hdl,PreOsdi}`; `rd/mod.rs::Block::loads` |
| A18 | OSDI binary load | ng (`.control` / `pre_osdi f.osdi` / `.endc`), va (`load`) | A compiled binary, no source behind it: only ngspice and VACASK take it (W0004 elsewhere). Loads given beside the text (Python) ride in `Source::loads`. | `Construct::OsdiLoad`; `rd/control.rs::ControlLine::PreOsdi`; `Deck::given_loads` |
| A19 | VACASK implied loads | va | `spice/<m>.osdi` loads are implied by SPICE-kind models (not kept); `resistor.osdi`, `capacitor.osdi`, `inductor.osdi` are VACASK's own linear RCL (kept as `own_loads`, only VACASK re-emits). Top loads sorted by path. | `rd/named.rs::named_statement`; `Block::own_loads` |
| A20 | `embed "file" <<<MARK … >>>MARK` | va | Long-string payload written to a file; if nothing loads/includes the file it is a script artefact. | `tok::KwEmbed`; `Lexicon::long_strings`; `rd/named_control.rs::vacask_script` |
| A21 | Global nodes | ng es hs xy lt (`.global a b`), sp va (`global`) | Deck-wide list, deduplicated; Spectre `global 0 …` (the leading `0` is ground, skipped). | `ast::Tag::Global`; `Deck::globals` |
| A22 | Xyce `$G_` global-node prefix | xy | Nodes named `$G_…` are global without a card; rewritten for targets where `$` starts a comment. | `beh::NetlistRules::Xyce`; `ws/globals.rs` |
| A23 | Ground declaration | va (`ground 0`) | `ground 0` is the default; `ground (a b)` has no field today (kept text). Python API: `gnd` is `0` only at the top level. | `tok::KwGround`; `rd/named.rs`; `src/python/subcircuit.rs` |
| A24 | Kept text, unread | all | A statement no rule reads: `Verbatim` (P0001, with its lexicon index), a line of a block kept whole (`Line`), `Raw::{Line,Lines,Joined}`, `Kept::{Raw,Unparsed}`. The IR needs "opaque statement in dialect X" with its family. | `ast::Tag::{Verbatim,Line}`; `rd/mod.rs::{Raw,Kept}`; `rd/refused.rs::RAWKIND` (92 dot words) |
| A25 | Kept-text classes for writers | all | ForeignText (another family's: refused W0001), PrivateText (only its own simulator has it: W0039), InertText (changes no number: left out W0336), copy on CDL (W0027), drop on CDL/Verilog (W0028/W0029). | `cap::Construct::{ForeignText,PrivateText,InertText}`; `beh::Behaviours::kept_words`; `rd/refused.rs::kept_construct` |
| A26 | Blocks kept whole | hs xy (`.data … .enddata`), hs (`.variation … .end_variation`) | Lines kept verbatim as a block. | `ast::Tag::Data`; `lex::SPICE_KEYWORDS` |
| A27 | Protected regions | hs | `.protect/.unprotect/.prot/.unprot` are raw dot words (kept text). | `rd/refused.rs::RAWKIND` |
| A28 | Kit (multi-file deck) | all | Files reached through includes, `.lib` calls, `include … section=`, Verilog-A sources; each file its own tree; include targets resolved to (file, section); depth cap 16, file cap 4096; HSPICE `.option search` directories (up to 40). Written per file, then re-pathed. | `res::{Kit,File,FileId,Target,DEPTH_MAX,FILES_MAX}`; `r/relink.rs`; R0006–R0010 |
| A29 | Text encoding | lt | UTF-16 / Latin-1 netlists; `µ` as a scale suffix. | `src/import.rs` (decode); `d/ltspice` SCALES |
| A30 | Xyce `.PREPROCESS` | xy | `REMOVEUNUSED` (drop listed element letters whose terminals are one node), `ADDRESISTORS` (second netlist request: dropped W0023), `REPLACEGROUND TRUE/FALSE` (TRUE renames ground nodes: kept text). | `beh::DotRule::Preprocess`; `rd/preprocess.rs` |
| A31 | Plugin (user-defined) dialect | — | A `DialectDef` registered at run time extends a built-in; may declare head/master/option maps, hooks, its own W0201/W0202 codes, `runs_on`. The IR target set is open. | `src/syntax/def/{mod,plugin,freeze}.rs`; `r/def_pass.rs`; `diag` W0201, W0202 |
| A32 | Statement order / placement | all | Today's plan places statements by kind (loads, globals, functions, includes, params, models, subckts, sections, body, testbench, `.end`) but keeps element order, `.include`/`.lib` order, testbench read order. | `rd/mod.rs::{Deck,Block,Block::elements}`; `ws/mod.rs` spine |
| A33 | `.title` card | ng | A title given by a dot card rather than line 1. | NG §3 (2.4–2.10); `RAWKIND` `title` |
| A34 | `.incpslt` | ng | An include whose file is read in PSpice compatibility: the included file needs another lexicon than its includer. Kept text today. | NG §3 (2.9) |
| A35 | `.pre_osdi` dot card | ng | Dot-card twin of the control `pre_osdi` command. | NG §3 (9); `RAWKIND` `pre_osdi`, `osdi_include` |
| A36 | `.spiceinit` `osdi` lines | ng (IHP kit) | OSDI loads that live outside the deck (init file); the kit must map each to its `.va` source for Spectre/VACASK (194 corpus lines). | NG §2 row 31, §5.4 |
<!-- A-end -->

## B. Hierarchy

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| B1 | Subcircuit definition | ng es hs xy lt (`.subckt … .ends`), hs (`.macro … .eom`), sp (`subckt name (ports) … ends name`, `inline subckt`), va (`subckt name(ports) … ends`) | Name, ordered ports, parameter defaults (SPICE: on the head; Spectre/VACASK: `parameters` inside), body. | `ast::Tag::Subckt`; `rd/cards.rs::Head{ports,params}`; `res::DefKind::Subckt{ports,params}` |
| B2 | `PARAMS:` separator | ng xy lt (written `PARAMS:`), hs (bare `k=v` after ports) | Optional keyword before the head's defaults. | `Spelling::subckt_params`; `rd/cards.rs::is_params_colon` |
| B3 | Nested subcircuit definitions | ng (manual §2.5) | Today hoisted as `<parent>__<name>` with the parent's references renamed; IR must decide scoped defs vs hoist. | `rd/blocks.rs` (`Kind::Subckt{renames,nested}`) |
| B4 | Subcircuit-local `.param` | ng es hs xy lt sp va | Locals separate from head defaults. | `Block::locals` |
| B5 | Subcircuit-local `.model` | ng hs xy (?) | Models declared inside a subcircuit, seen only from inside; inherited into nested scopes. | `Block::models`; `ws/values.rs::inherited_models`; `res::Def::within` |
| B6 | Subcircuit-local `.func` | ng hs xy | Kept local only if it reads a subcircuit parameter or shares a name; else moved to the top. LTspice, Spectre, VACASK refuse (W0106). | `Construct::LocalFunc`; `Block::funcs`; `rd/funcs.rs` |
| B7 | Unreadable subckt head | ng… | Head kept as text, body dissolved into the parent, `.ends` kept raw (old-reader quirk). The IR needs an error node, not a dissolve. | `rd/blocks.rs::Opened::Dissolve` |
| B8 | Subcircuit instance (X) | ng es hs xy lt | `X<name> n1 … master [PARAMS:] k=v`; the master is the last bare word before the parameters, or one last word after them (sky130's late name). | `dev::Model::Subckt`; `rd/elements.rs::Card::Inst{name,ports,master,params}` |
| B9 | Named instance of a subckt | sp va | `x1 (a b) master k=v`; VACASK requires the parentheses; master may be a model, a subckt, a built-in master (Spectre) or a Verilog-A module. | `rd/named_elements.rs::named_inst`; `res::Binding{Primitive,Def,Unknown}` |
| B10 | Instance multiplicity `m=` on X | ng hs xy (multiplies), es (ignored), va (`$mfactor`) | `m` on an X line is the multiplicity unless the subckt declares a parameter `m`; ESPice ignores it (W0047); Xyce `M` renamed where it clashes. | `beh::Behaviours::subckt_multiplicity`; `beh::NetlistRules`; `ws/values.rs::m_rename`; W0047 |
| B11 | Parameter passing value forms | all | number, `{expr}`/`'expr'`, bare parameter name (Xyce reads a bare name as a string: must be braced), string. Xyce may give an instance another instance's parameters (W0140). | `ws/xyce_cards.rs`; W0140 |
| B12 | Port-count / master checks | all | Node count ≠ ports (R0004); unknown master (R0001); name declared twice (R0002); master naming different kinds (R0003) or a param/func (R0005). | `diag` R0001–R0005 |
| B13 | Conditional netlist | ng hs (`.if (c)` / `.elseif (c)` / `.else` / `.endif`), sp (`if (c) { } else { }`), va (`@if c` / `@elseif` / `@else` / `@end`) | Chain of (condition expr, body); bodies hold elements, instances, models, nested chains. Xyce and LTspice have none (W0013). SPICE `.if` whose condition is not one parenthesised group is kept whole. | `ast::Tag::{If,Else}`; `rd/mod.rs::Cond{branches}`; `Construct::Conditional` |
| B14 | `.param` or load inside a branch | sp | Only Spectre takes a parameter or a load inside a conditional (W0014 elsewhere; ngspice manual: no `.SUBCKT`, `.INC`, `.LIB`, `.PARAM` in a branch). | `Construct::BranchParam`; `src/python/block.rs` |
| B15 | Brace block statement | sp | `name master … { … }` kept whole unless an analysis (`sweep`, `montecarlo`) or `altergroup`. | `rd/named.rs::named_card` (quirk `Block`) |
| B16 | Element naming | all | SPICE: type letter is the first byte of the name; Spectre/VACASK: free name + master. Name map adds/strips the letter (`in1 (a 0) vsource` ↔ `Vin1`); renames on a letter change (B → E/G on HSPICE; Laplace lift `EB1`/`GB1`/`AB1`), uniquified `_1`, `_2`. | `r/names.rs::Names`; `rd/named_elements.rs::strip` |
| B17 | References to elements by name | all | F/H/W controlling source, K's inductors, `i(V1)` in outputs and expressions, `probe=`/`ctlinst=`/`dev=`/`iprobe=`, `save dev:p`: resolved per scope and spelled as the declaration. | `dev::Device::refs`; `rd/named.rs::declared`; `Names::refs` |
| B18 | Hierarchical names | all | Separator `.` (default) or `:` (Xyce); normalised result names use `.`. | `beh::Behaviours::hier_sep`; `src/normalize.rs` |
| B19 | Name escaping | sp (`\` before a byte), va (`'…'` name parts) | Names that are no identifier in the target must be quoted/escaped; Verilog escaped identifiers and keywords. | `Lexicon::name_escape`; `lex::Quote::Name`; `ws/verilog.rs` |
| B20 | Name bytes | per dialect | `$` starts/continues names (all); `.` (SPICE), `:` (Xyce), `!` (Spectre); glued word tokens `net-1`, `vdd!`, `n<3>`, `10%3`; Xyce names hold `#`, `[`, `+`, backtick. | `Lexicon::{ident_start,ident_continue}`; `ast::Tag::Word`; `rd/rejects.rs::pieces` |
| B21 | Scope model | all | Top / Subckt / Section / Branch; libraries, regions, `.control`, model groups, `statistics` are no scope. | `res::Scope`; `rd/mod.rs::Scope{Top,Subckt,Section}` |
| B22 | VACASK default top instance | va | `__topinst__` names the top; `alter instance("__topinst__") k=v` sets top parameters. | `rd/named_control.rs::vacask_alter` |
| B23 | VACASK promoted subcircuits | va | `elaborate circuit("a", …)` before the first analysis makes portless top-level subckts the top circuit(s); a second `elaborate` = a second circuit in one run (W0122). | `rd/named_control.rs::{vacask_promoted,sole_elaborate}`; `Construct::VacaskElaborate` |
| B24 | Verilog-A module instance | sp (by module name), ng (`N` card on a `.model` of the module), va (model of a loaded module) | Instance of a loaded module: nodes by the module's ports, parameters by its names; `$mfactor` ↔ `m`. | `Construct::OsdiInstance`; `Card::Semi{letter:'N'}`; `res::DefKind::VerilogA{ports,params}`; `rd/named_elements.rs::va_module` |
| B25 | CDL block layout | cdl (writer) | Each block (top, then each subckt) is one `.SUBCKT … .ENDS`; elements sorted by name; a comment per Verilog-A load; top models; `.END`; `*.PININFO`. | `ws/cdl.rs` |
| B26 | Structural Verilog module | vlog (writer) | One `module` per subckt: `inout` ports, `wire` nets, numeric `parameter` defaults, instances `master #(.p(v)) name (.port(net))` by name where the master is defined, by position otherwise; every non-subckt element is a `// W0029` comment. | `ws/verilog.rs` |
| B27 | Generate / array constructs | none seen in code | No device arrays, `generate`, iterated instances in the reader; LTspice `.machine` state machines are private text (L area). | — |
| B28 | `=` as equality in a condition | ng | `.if (select2 = 2)`: a single `=` is equality in ngspice conditions; must become `==`. Unread today (W0032, P0002). | NG §2 row 26 |
| B29 | Conditional inside a subcircuit on its parameter | ng hs sp va | Per-instance branch selection (IHP `.if (…ng…) / .else / .endif` in `.subckt`); the reader keeps the branch raw inside a subckt for sp/va. | NG §2 row 34 |
<!-- B-end -->

## C. Parameters & expressions

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| C1 | Parameter definition | ng es hs xy (`.param`, `.parameter`, `.parameters`), lt (+ `.params`), sp va (`parameters a=1 b=2`) | Name = number / expression / string; several per card. | `ast::Tag::Param`; `lex::SPICE_KEYWORDS`; `rd/cards.rs::param_view` |
| C2 | Redefinition winner | — | Same name defined twice in one block: ngspice and HSPICE keep the last, Xyce the first, LTspice/VACASK/Spectre refuse (Spectre `redefinedparams=warning` keeps the last). IR should hold one definition after resolving by source rule. | `rd/cards.rs::{param_winners,keep_winners}`; R0002 |
| C3 | Xyce `.global_param` | xy | A parameter Xyce re-evaluates during the run; equals `.param` unless it reads a swept variable (`temp`, `vt`, `freq`, `hertz`, `time`), then kept as text. Written back as `.global_param` on Xyce. | `rd/cards.rs::global_param`; `rd/refused.rs::global_swept` |
| C4 | ngspice `.param` oddities | ng | A `+` word between assignments; value `{1+x} + x*AGAUSS(0,1,1)` (brace group then operator). | `rd/cards.rs::{param_card,val_p_ok}` |
| C5 | `.csparam` | ng | Parameter copied into a control-script vector; inert outside ngspice. | `d/ngspice` kept_word |
| C6 | Function definition | ng xy lt (`.func f(a,b) {body}` / `= body`), hs (`.param f(a,b)='body'`), sp (`real f(real a, real b) { return e; }`) | Name, formal args, body expr. VACASK has none (W0106); Spectre functions top-level only. Functions calling statistical draws are inlined per call site. | `ast::Tag::Func`; `rd/mod.rs::Func{Param,Card,Real}`; `r/spelling.rs::FuncForm{Func,Param,Real,None}`; `rd/unlift.rs::Fix::Inline` |
| C7 | Expression delimiters | ng xy lt `{…}`; ng hs xy `'…'`; hs `"…"` as expression; sp va bare | Delimiters are spelling only; HSPICE braces are name bytes; HSPICE quotes anything it would read as words. | `ast::Tag::Delimited`; `lex::Quote{Plain,Expr,Name}`; `Lexicon::{expr_braces,string_expr}`; `beh::ValueQuoting` |
| C8 | Binary operators, ngspice | ng | `\|\|`(2) `&&`(3) `== != <> <= >= < >`(4) `+ -`(5) `* / % \`(6) `^ **`(7); all left-assoc. | `d/ngspice` infix rows |
| C9 | Binary operators, HSPICE | hs | As ngspice less `<>`, `%`, `\`; no unary `!`. | `d/hspice` |
| C10 | Binary operators, Xyce | xy | `\|\| \| && &`(2) `^`=XOR(3) comparisons incl `<>`(4) `+ -`(5) `%`(6) `* /`(7) `**`(9, right); prefix `+ - ~`. | `d/xyce` |
| C11 | Binary operators, LTspice | lt | `& \| ^` Boolean (2) comparisons (3) `+ -` (4) `* / %` (5) `**` (6, right); prefix `+ - ! ~`. | `d/ltspice` |
| C12 | Binary operators, Spectre | sp | `\|\|` `&&` `== !=` `<= >= < >` `+ -` `* / %` `**` (right); prefix `+ - !`; no `^`. | `d/spectre` |
| C13 | Binary operators, VACASK | va | `\|\|` `&&` `\|` `^`(XOR) `&` `== !=` `<= >= < >` `<< >>` `+ -` `* /` `**` (right); prefix `+ - ! ~`; no `%`. | `d/vacask` |
| C14 | Neutral operators | nx | `?:`, `\|\|`, `&&`, `== !=`, `< <= > >=` (non-associative: P0022), `+ -`, `* / %`, sign, `**` (right, above the sign: `-2**2` = -4); no `<>`, `\`; `^` refused (P0023). | `d/spicerack` |
| C15 | `^` meaning | pow: ng hs; XOR: xy lt va; none: sp | Must be resolved to power or XOR at read time (W0002 when a target rereads it). | `lex::Caret{Pow,Xor,None,Refused}`; `Construct::CaretReread` |
| C16 | Power value law | — | ngspice (and ESPice): `abs(base)**exp` (`(-2)**3` = 8); Xyce, LTspice, HSPICE, Spectre keep the sign for a whole exponent (-8); Xyce gives the real part of the complex power otherwise. Functions `pow`, `pwr`, `pwrs` differ likewise. | `ws/powers.rs::Law`; `ws/ternary.rs::{xyce_power_calls,ngspice_power_calls}` |
| C17 | Power grouping | — | `a**b**c`: ngspice 64 (left), Xyce/LTspice 512 (right); `-a**c`: ngspice/Xyce -4, LTspice +4. Source grouping must be made explicit. | `ws/powers.rs::Grouping`; `lex::Ops::{right,prefix_bp}` |
| C18 | Ternary grouping | ng hs xy lt sp va | C: `a?b?c:d:e` = `a?(b?c:d):e`; ngspice `{}` evaluator reads it `a?c:e`, deeper then-nesting no one way (W0034 note, W0035 refusal). Xyce needs a blank before `:` (else a hierarchical name). | `beh::ExprEval{C,Ngspice,Xyce,Hspice,Ltspice}`; `ws/ternary.rs::{Read,Nest}`; `lex::Nesting` |
| C19 | `if(c,a,b)` | xy lt | Function form of the ternary; ngspice `{}` has none. | `ws/ternary.rs::spell_if` |
| C20 | `log` base | natural: ng hs lt sp va; base 10: xy | Rename `log` ↔ `ln`/`log10` between families. | `beh::ExprEval`; `ws/ternary.rs::rename_call` |
| C21 | `nint` rounding | ng (to even), xy (away from zero) | Neutral: `round` half away from zero, `int` truncates. | `ws/ternary.rs::spell_nint`; P0026 |
| C22 | `limit` | ng: `limit(nom, var)` a distribution; xy lt: `limit(x, lo, hi)` a clamp | Same word, two meanings by arity and dialect. | `rd/unlift.rs::Lift::{Limit,Clamp}`; `ws/ternary.rs::spell_clamp` |
| C23 | Statistical draw calls | ng hs xy (`agauss gauss aunif unif limit`), va (`agauss gauss aunif unif`) | Draw vs nominal: ngspice draws at every reference; Xyce and HSPICE give the nominal outside sampling; Xyce `rand()` = 0.5; Xyce 2-arg `agauss` has sigma 1. LTspice refuses (W0003; its `gauss(x)` differs); Spectre lifts to `rack_*` + `statistics`. | `Construct::StatisticalCall`; `cap::Rule::{RackVars,VacaskCalls}`; `ws/ternary.rs::nominal` |
| C24 | Time integral / derivative | xy (`sdt`, `ddt`), ng B source (`ddt`, `idt`?), sp (`ddt`, `idt`) | `sdt` on ngspice refused (W0045); `ddt` only in a B source or a charge (P0025); Spectre bsource with `ddt`/`idt` has no SPICE B form. | `ws/checks.rs`; `rd/named_masters.rs::b_expr` |
| C25 | Probe calls in expressions | all | `v(a)`, `v(a,b)`, `i(src)`; Xyce also `vr vi vm vp vdb ir ii im ip idb n p w`; arguments are node/device names, never expressions; Spectre bsource `i("v1:0")`; VACASK `i(v1)`. | `ast::Tag::Probe`; `lex::Ops::probes`; `wn/bsource.rs` |
| C26 | Circuit temperature name | ng hs `temper`; xy lt sp `temp`; va `$temp` | One neutral symbol. | `beh::Behaviours::temp_name` |
| C27 | Time / frequency names | ng `time`, `hertz`; sp `$time`; va `$abstime`; xy `time`, `freq`, `hertz`, `vt` | Neutral symbols; `hertz` has no Spectre bsource form. | `wn/bsource.rs`; `rd/refused.rs::global_swept` |
| C28 | Built-in constants | va (`M_E M_LOG2E M_LOG10E M_LN2 M_LN10 M_PI M_TWO_PI M_PI_2 M_PI_4 M_1_PI M_2_PI M_2_SQRTPI M_SQRT2 M_SQRT1_2 M_DEGPERRAD P_Q P_C P_K P_H P_EPS0 P_U0 P_CELSIUS0`), ng (`pi`), sp (`M_PI`) | Written as numbers off the source. | `beh::Behaviours::builtin_constants` (22 rows) |
| C29 | Functions with one meaning ng B ↔ Spectre | — | `sin cos tan asin acos atan sinh cosh tanh exp sqrt abs min max pow` (plus `asinh acosh atanh floor ceil` in the bsource writer). `log`/`ln` excluded (bases differ). | `rd/named_masters.rs::SAME_FUNCS`; `wn/bsource.rs` |
| C30 | Functions needing rewrite | — | `if`, `log`, `nint`, `limit`, `rand`, `pwr`, `pwrs`, `pow`, `sdt`, `table`, `pwl`, `agauss…`: each a row of `REWRITES`. | `ws/values.rs::REWRITES`; `ws/ternary.rs` |
| C31 | Neutral function set | nx (planned) | P0026 cites `render::spelling::Func` — not present in batch-22 `spelling.rs` (no `Func` enum in any worktree); the set is in D-neutral-expressions. | `diag` P0026; `r/spelling.rs` (absent) |
| C32 | Number literal shape | all | Mantissa, exponent letters (`eE`; `dD` too on ng, hs), one scale suffix, then a unit tail. | `lex::Numbers{exponent,scales,tail,arithmetic}` |
| C33 | Scale suffixes, SPICE | ng hs xy lt | `t g meg k m u n p f` + `mil` (25.4e-6); `a` atto (ng hs; not xy lt); `x` = mega (hs xy); LTspice `µ`. Case-blind: `M` is milli. | `d/{ngspice,hspice,xyce,ltspice}` scale rows; `lex::Scale{Exp,Mil}` |
| C34 | Scale suffixes, Spectre | sp | Case-sensitive `P T G M K/k _ %/c m u n p f a` (`M` mega, `m` milli, `_` = 1, `%`/`c` = 1e-2, `P` = 1e15); units may follow; a scale may follow an exponent (`1e3k`). | `d/spectre` SCALES; `lex::Tail::Letters` |
| C35 | Scale suffixes, VACASK | va | `meg M x X` mega, `m` milli, `mil`, `u n p f a k K G T`; letters after the scale; `1e3k` an error; `0x…` hex not decoded. | `d/vacask` SCALES |
| C36 | Unit tail | ng hs xy: letters/digits/`_` (+`.` on ng hs); lt: `6K34` = 6.34k; sp va: letters only; nx: none (`1kohm` is no number, P0020) | A number's trailing unit is ignored text; must not survive into another dialect. | `lex::Tail{Spice,Ltspice,Letters,None}` |
| C37 | Number arithmetic | ng (digit-by-digit `INPevaluate`), others decimal | `100u` is 9.999999999999999e-5 on ngspice; the writer keeps "read with ngspice arithmetic" to spell a number back to the same double. | `lex::Arithmetic{Ngspice,Decimal}`; `beh::NumberReading`; `r/mod.rs::Numbers`; `Spelling::ngspice_numbers` |
| C38 | Unreadable number | all | A token that starts like a number but the suffix table does not read: value NaN (P0005). | `tok::Tokens::number_values` |
| C39 | Strings | all | `"…"` with/without escapes, protecting comment bytes or not (Xyce: `;` inside quotes still a comment), across breaks (HSPICE); VACASK long strings; string-valued parameters (`version="4.8"`, `type="pulse"`). | `ast::Tag::Str`; `lex::Quoting{escapes,protects,across_breaks}` |
| C40 | Vectors / lists | sp `[a b]` (signed atoms), va `[a, b]` (expressions) and `{a, b}`, va `["n"; v; …]` | Typed arrays of numbers, names or strings. | `ast::Tag::{Vector,List}`; `lex::Ops::index` |
| C41 | Number sequences | ng hs xy | Comma vector `ic=0,-5`; blank-separated model matrix `c=1e-10 -6e-11`; Xyce `TC=a,b`. | `ast::Tag::Seq`; `rd/values.rs::{vtext,mtext}` |
| C42 | Indexing | va | `x[i]`. | `ast::Tag::Index` |
| C43 | Value classes today | — | `Val::{Num(f64),Expr(String),Raw(String)}`; waveform `Num::{Num,Expr}`. Expressions are kept as text with token fixups, not as trees, in the views. | `rd/values.rs::Val`; `rd/sources.rs::Num` |
| C44 | Flags (valueless parameters) | ng hs xy lt | `off`, `on`, `thermal`, bare option words; Spectre takes `name=value` only (W0101), mapped rows give `region=off`, `isnoisy=yes/no`. | `dev::FLAGS`; `r/models.rs::Param::{Flag,YesNo}` |
| C45 | Expression parse failures | all | `Bad` node (P0002), unclosed delimiter (P0008), depth cap (P0006); text kept; W0032 when written for another dialect. | `ast::Tag::Bad`; `diag` |
| C46 | Domain differences | — | Constant out of domain (P0024); expressions whose domain targets evaluate differently (W0137); Xyce domain rewrites. | `ws/ternary.rs::xyce_domains`; `diag` W0137 |
| C47 | Token fixups | — | Today rewrites are fixups keyed by token: respell a reference, lift a statistical call, inline a function. An owned IR replaces these with tree edits. | `rd/unlift.rs::{Fix,Lift,Step}` |
| C48 | `.params` / `.para*` spelling | ng (any `.para…` card is `.param`), lt (`.params`) | Alias only. | NG §2 row 32 |
| C49 | `&` parameter form | ng (deprecated) | `&name` substitution. | NG §3 (2.11) |
| C50 | `%` and `\` semantics | ng | `%` modulo, `\` integer divide; Xyce has no `\`; each target must spell int-divide/fmod. | NG §3 (2.11.5) |
| C51 | ngspice numparam function set | ng | `sqrt … ternary_fcn`, `nint` (half to even), `int`, `sgn`; statistical `gauss agauss unif aunif limit`. | NG §3 (2.11.5) |
| C52 | ngspice B-source function set | ng | `u`, `u2`, `uramp`, `pwl()`, `par()`, `hertz`, `time`, `temper`, `ddt`, `idt`, `ln`, `log`, `log10`, `pow`, `**`, `^`, `pwr`. | NG §3 (5.1) |
| C53 | ngspice B-source math laws | ng | `pow`/`**`/`^` = pow(abs(x), y); `exp` linear past argument 14; `log`, `ln`, `sqrt` take abs(x): silent number differences on every other target for negative or large arguments. | NG §3 (5.1), §5.6 |
| C54 | Bare parameter name as a value | ng | `area=DEV_A`, subckt default `r_rsh0=rsh_nplus_u_m`: a value that is one identifier; Xyce needs braces. | NG §2 rows 33, X4 |
| C55 | Compatibility mode changes expression meaning | ng | `.options ngbehavior=hs` (etc.) changes expression and power semantics; the reader must apply it to the deck, not only note it. | NG §3 (12.14) |
| C56 | Constant folding of analysis arguments | ng | `.TRAN 0.05MS 'ttime'`, `.tran 1n {tt}`, `.temp {envtemp}`: analysis numbers given as parameter expressions (global constants). | NG §2 rows 20, 21 |
| C57 | LTspice B-source function set | lt | `absdelay delay ddt idt sdt idtmod buf inv uramp u dnlim uplim limit table rand random white noise if select flat mc gauss round …`. | LH §5 |
| C58 | LTspice constants | lt | `pi BOLTZ ECHARGE PLANCK KELVIN GMIN`, `temp`, `time`; which need renaming per target is unsettled. | LH §5 |
| C59 | HSPICE expression functions | hs | `sin cos … pow pwr sqrt log log10 exp db abs sgn sign int nint floor ceil min max`; ternary without `if`; `v()`, `i()`, `temper`, `time`, `hertz`. | LH §6 |
| C60 | `^` scoped by context | lt | `^` is XOR in LTspice expressions but power inside `Laplace=`: operator meaning depends on the slot. | LH L13, §7 |
| C61 | Xyce expression functions (same on ngspice) | xy | `abs ceil floor sqrt exp ln log10 sin cos tan asin acos atan sinh cosh tanh asinh acosh atanh min max pow pwr uramp`, ternary, comparisons. | XY §2d |
| C62 | Xyce functions needing a rewrite | xy | `%`, `fmod`, `int`, `nint`, `sgn`, `sign(x,y)`, `stp`, `arctan`, `atan2`, `pwrs`, `m()`, `r()/re()/img()/ph()/db()` (complex parts), `ddx(f,x)` (symbolic derivative: no exact rewrite), `sdt`, `limit(x,lo,hi)`. | XY §2d |
| C63 | Xyce table functions | xy | `table(x, pairs…)`, `tablefile`/`table("file")`, `fasttable`, interpolators `spline akima cubic wodicka bli` (only linear maps to `pwl()`). | XY §2d |
| C64 | Xyce source functions in expressions | xy | `spice_pulse(...)`, `spice_sin(...)`, `spice_exp(...)`, `spice_sffm(...)`; `schedule(t0,dt0,…)` (only in `.TRAN`). | XY §2d, A6 |
| C65 | Xyce special variables | xy | `TIME`, `TEMP`, `VT`, `FREQ`/`HERTZ`, `PI`; MPDE `time1`/`time2`. | XY §2d, S8 |
| C66 | Xyce logical operators | xy | `~`, `&`, single-pipe OR, `^` (XOR) inside `IF(…)`. | XY E9, §2d |
| C67 | Xyce identifier characters | xy | `.FUNC #func(x)`; names with `#`, backtick, `!`; node names holding `+`/`-` (`S+`, `P-`). | XY G7, E11 |
| C68 | `POLY(n)` as an expression | xy | Inside `B … V=`/`I=` and inside a `.func` body. | XY E16, E17 |
| C69 | Time- or frequency-dependent parameters | xy | `.GLOBAL_PARAM x={…time…}`, `{…TEMP…}`, `{log10(freq)+1}`: a parameter that is a run-time signal, not a constant. | XY G1–G3 |
| C70 | First-definition-wins switch | xy | `-redefined_params` command-line option changes which duplicate `.param` wins. | XY G4 |
| C71 | VACASK function set | va | Trig, hyperbolic, `exp`, `log` (= ln), `log10`, `sqrt`, `pow`, `abs`, `floor`, `ceil`, 2-argument `min`/`max`, `atan2`, `hypot`, `fmod`, `round`, `integer`, `sgn`, `sign`, `is*` predicates, vector/list functions, `readfile`, `randunif`; bit operators `<< >> & ^ ~` (fold when constant). | VS §2 Expressions |
| C72 | Functions undefined on some targets | sp va | `fmod`, `hypot`, `atan2` undefined on ngspice; `M_*`/`P_*` constants undefined on ngspice and Xyce (and lowercased wrongly for VACASK); `%` is a VACASK parse error. | VS §2b Expressions (measured) |
| C73 | Control-block variables | va | `var name=expr`; a sweep over a variable; analysis arguments as expressions of variables (`stop=tend`); an inner sweep bound from an outer variable (`to=vsup`). | VS V8–V10, V15 |
| C74 | Vector-valued parameter | va | `sweep … parameter="vec" values=[[…];[…]]`: a sweep whose values are vectors. | VS V16 |
| C75 | Neutral expression language (D16) | nx | Identifiers `[A-Za-z_][A-Za-z0-9_.]*`; probes `v(n)`, `v(n1,n2)`, `i(dev)`; reserved `time`, `temp`, `pi` (no physical constants: their values differ by simulator); reals only; `%` = C `fmod`; `x**y` for x<0 real only for integer y (else a domain error); `log` natural; the ternary evaluates only its chosen branch. | D-neutral-expressions §2 |
| C76 | Neutral function set | nx | Core `abs sqrt exp log log10 pow min max floor ceil`; integer `int round sgn`; trig `sin cos tan asin acos atan atan2 sinh cosh tanh asinh acosh atanh hypot`; `fmod`, `limit(x,lo,hi)`, `if(c,a,b)`; statistical `agauss aunif gauss unif`; context-only `ddt(x)`, `table(x, x1, y1, …)`; `idt`/`sdt` excluded. | D-neutral-expressions §2 |
<!-- C-end -->

## D. Devices

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| D1 | Resistor | ng es hs xy lt (`R`), sp (`resistor r=`), va (`resistor` built-in, `sp_resistor`) | 2 nodes; value positional, `r=`, or absent; model word among the args (only resolve can tell it from a value); Xyce `R1 a b RMOD 1k` (model before value); Xyce valueless R = 1000 Ω. | `dev::SPICE` row `r`; `rd/elements.rs::rcl`; `Card::Rcl{letter,name,n1,n2,value,model,params}` |
| D2 | Resistor instance parameters | ng | `resistance r ac temp dtemp l w m tc tc1 tc2 tce bv_max scale noisy noise`; Spectre names `r trise l w m tc1 tc2 isnoisy`; `scale` folds into `m`. | `r/models.rs::R_INSTANCE` |
| D3 | Voltage-dependent resistor | ng | `R` whose value reads `v()`, `i()`, `temper`, `hertz`, `time` is run by ngspice as a B source keeping `tc1 tc2 m`; Spectre `resistor r=100*(1+v(n1,n2)/2)`. | `wn/params.rs`; W0113 |
| D4 | Capacitor | ng es hs xy lt (`C`), sp (`capacitor c=`), va | As D1; instance `capacitance cap c ic temp dtemp w l m tc1 tc2 bv_max scale`. | `dev::SPICE` row `c`; `r/models.rs::C_INSTANCE` |
| D5 | Charge-defined capacitor | ng hs es (`q='expr'`), xy (`Q={expr}`), lt (`Q=expr`), sp (`bsource q=expr`), va (`i=ddt(q)`) | `C1 a b q=<expr of v>`; one construct, five spellings. | `Card::Charge{name,n1,n2,charge,value,params}`; `Construct::ChargeCapacitor`; `Alt::{ChargeQuoted,ChargeBraced,ChargeBare,BsourceCharge}`; W0103 |
| D6 | Capacitor `ic=` | ng (only under UIC), xy lt (held in the operating point), va (no parameter: entry of the tran `ic` list) | Initial condition semantics differ by source (W0049). | `beh::NetlistRules`; `ws/initial.rs`; `wn/vacask_ic.rs` |
| D7 | LTspice parallel units | lt | Capacitor `x<n>` (n units in parallel, anywhere on the card) scales C·n and each parasitic. | `rd/parallel.rs` |
| D8 | LTspice part parasitics | lt | C: `Rser Lser Rpar Cpar RLshunt`; L: `Rser Rpar Cpar`; V: `Rser Cpar`; plus ratings with no simulation meaning (`V Irms Ipk mfg pn type pppkg mtbf`). Other targets get explicit R/L/C on an internal node. | `beh::Parasitics{Elements,Ltspice,Netlist}`; `ws/parasitics.rs`; `wn/parasitics.rs`; `Card::Source::parasitics` |
| D9 | Inductor | ng es hs xy lt (`L`), sp (`inductor l=`), va | As D1; an inductor's model has no mapped kind (RclModel refused off ngspice semantics). | `dev::SPICE` row `l` |
| D10 | Mutual coupling | ng es hs (2 inductors per `K`), xy lt (N inductors, all pairs), sp (`mutual_inductor coupling= ind1= ind2=`), va (`mutual k= ind1= ind2=`, no nodes) | List of inductor refs + k; pairwise vs one card. | `Card::Coupling{name,ls,refs,k}`; `beh::Coupling{Pairs,OneCard}` |
| D11 | Nonlinear-core coupling | xy | `K … k model` (`.MODEL … CORE`): private (ngspice/LTspice linear only; HSPICE Jiles-Atherton another formulation). | `d/xyce` kept_word `k`; `rd/refused.rs::private_line` |
| D12 | Independent V source | ng es hs xy lt (`V`), sp va (`vsource`) | 2 nodes; leading value, `dc v`/`dc=v`, `ac mag [phase]`/`ac=`, waveform; DC defaults to the wave's value at t=0 (SFFM on ngspice: 0; TRNOISE/TRRANDOM/AM: 0). | `Card::Source{letter,name,np,nm,value,ac_mag,ac_phase,wave,port,parasitics}`; `rd/sources.rs::Spec{Dc,Ac,Wave,Pair}` |
| D13 | Independent I source | ng es hs xy lt (`I`), sp va (`isource`) | As D12, no port. | same |
| D14 | Current probe | sp (`iprobe`) | A 0 V source in series; SPICE uses a 0 V `V`. | `rd/named_masters.rs` |
| D15 | RF port | ng (`V … portnum n z0 Z`), xy (`P n+ n- port=n z0=z`), sp (`port num= r= dc= mag= phase= type=`), va (native cell) | Port number + reference impedance (default 50). Spectre's `dc`/`mag` are across the terminated port (×2 to the source behind `r`). HSPICE, LTspice refuse (W0007). | `Card::Source::port`; `Construct::RfPort`; `Alt::XycePort`; `dev::XYCE_PORT`; `wn/sources.rs` |
| D16 | VCVS (E) linear | all | 4 nodes + gain; Spectre `vcvs gain=`; VACASK `vcvs gain=`. | `Card::Linear{letter,name,nodes,gain}`; `dev` rows |
| D17 | VCCS (G) linear | all | 4 nodes + gm (`gm=` Spectre, `gain=` VACASK); `m=` on G folds into the gain. | `rd/controlled.rs::multiplier` |
| D18 | CCCS (F) / CCVS (H) linear | all | 2 nodes + controlling V-source name + gain; Spectre `probe=`, `gain=`/`rm=`; VACASK `ctlinst=`, `gain=`; `m=` on F folds in; none on E/H. | `Card::Linear` (3 nodes); `rd/named_elements.rs::Keys` |
| D19 | POLY controlled source | ng es hs xy (`E/G/F/H … POLY(n) ctrls… coeffs…`), sp (`pvcvs pvccs pcccs pccvs coeffs=[…] probes=[…]`) | Dimension, control pairs or source names, coefficient list. Xyce also `B … V={POLY(n) v(a) … c…}` read as the E/F/G/H card. | `Card::Poly{letter,name,np,nm,controls,coeffs}`; `rd/devices.rs::{poly,b_poly}` |
| D20 | E/G expression source | ng es xy (`E … value={expr}`), hs (`E … vol='expr'`, `G … cur='expr'` + `SCALE= MAX= MIN=`) | Same as a B source with fixed output kind; HSPICE modifiers folded into the expression. | `Card::Behavioural`; `rd/controlled.rs::modified`; `Alt::VolCur` |
| D21 | E/G TABLE source | xy (`TABLE {expr} = (x,y) …`) | Piecewise-linear table, ends held, vertical step takes the later point; ngspice `value={pwl(expr, …)}`; ESPice ternary chain (≤ 5 segments, 1 probe); others W0102. ngspice/ESPice native `TABLE` rounds corners. | `Card::Table{current,name,np,nm,expr,points}`; `Construct::TableSource`; `Alt::{PwlValue,TernaryValue}`; `ws/table.rs` |
| D22 | Behavioural source (B) | ng es lt (`B n+ n- V=expr` / `I=expr`), xy (`V={expr}`), sp (`bsource v=`/`i=`), va (`b1 (p n) v=expr`, no master) | Output kind (V or I), nodes, expression to end of card (may hold probes, `time`, `temper`, user functions). ESPice B limits: 8 probes, 64 ops, 32 constants, depth 16, no `pwl()`/`table()`. | `Card::Behavioural{current,name,np,nm,expr,value}`; `Construct::BehaviouralSource`; `Alt::{BracedBehaviour,Bsource}`; `wn/bsource.rs`; W0102 |
| D23 | Laplace transfer source | lt (`Laplace=num/den` attribute), hs es (`E/G … LAPLACE c+ c- num… / den…`), ng (via XSPICE `s_xfer` + `.model`) | `Laplace(V(a[,b]), H(s))` with H rational in `s` (numeric coefficients, proper, non-constant denominator); Xyce/Spectre/VACASK refuse (W0011); unreadable H (W0010). | `Construct::Laplace`; `cap::Rule::{LaplaceXfer,LaplaceTable,LaplaceAttr}`; `ws/laplace.rs::Laplace` |
| D24 | Diode | all (`D` + model; sp `diode`; va `sp_diode`) | 2 nodes, model, area/flags/params; instance `area pj perim w l m lm lp wm wp temp dtemp ic off thermal`. | `Card::Semi{letter,name,nodes,model,params}`; `r/models.rs::D_INSTANCE` |
| D25 | BJT | all (`Q`; sp `bjt`; va `sp_bjt`, `vbic13`, `vbic13_5t`) | 3–5 nodes (substrate, thermal `tj`); split of nodes vs model needs the kit (`Q1 c b e mod 1.5`); flags `off on thermal`. | `dev` row `q` `Range(3,5)`; `res::Split`; `rd/devices.rs::semiconductor` |
| D26 | MOSFET | all (`M`; sp `mos1 mos2 mos3 bsim3v3 bsim4`; va `sp_mos1 … sp_bsim4v8`, `sp_vdmos`, `psp103va`) | 3–7 nodes (HSPICE bulk optional, BSIMSOI up to 7); VACASK instance params `instance_<p>` ↔ `<p>`, `$mfactor` ↔ `m`. | `dev` row `m` `Range(3,7)`; `rd/named_elements.rs::device` |
| D27 | JFET | ng hs xy lt (`J`), va (`sp_jfet1`, `sp_jfet2`) | 3 nodes + model. Spectre reader has no JFET kind. | `dev` row `j` |
| D28 | MESFET / HFET | ng hs xy (`Z`), va (`sp_mes1`) | 3 nodes + model; ngspice HFET types `nhfet phfet` (levels 5, 6) have no VACASK module. | `dev` row `z`; `d/vacask` spice_model_type |
| D29 | Voltage switch (S) | ng es hs xy lt; sp (`relay`) | 4 nodes + model; Python API: optional `ON`/`OFF` state word. | `Card::Switch{letter,name,nodes,model}`; `src/python/keywords.rs::SwitchState`; `wn/cards.rs` (relay p.625) |
| D30 | Current switch (W) | ng es xy lt (hs: `W` is the W element) | 2 nodes + controlling source + model. | `dev` row `w` |
| D31 | Lossless transmission line (T) | ng es hs xy lt; sp (`tline z0= td=`) | 4 nodes; `Z0=` + `TD=`, or `F=` [+ `NL=` default 0.25] (td = nl/f); Spectre `f/nl`, `len/vel`, loss terms have no field. | `Card::Line{name,nodes,z0,td}`; `rd/elements.rs::tline` |
| D32 | Lossy line LTRA (O) | ng xy lt | 4 nodes + LTRA model. HSPICE/Spectre/VACASK W0105. | `dev::LOSSY_LINE`; `Construct::LossyLine` |
| D33 | Uniform RC line URC (U) | ng | 3 nodes + model, `l=len [n=lumps]`. No reader rule (kept raw). On Xyce `U` is a digital device (private). | `dev::NGSPICE_OWN`; `src/python/lines.rs` |
| D34 | Single lossy line TXL (Y) | ng | `Y n1 0 n2 0 model [len=]`. On Xyce `Y<type>` is a device family (private). | `dev::NGSPICE_OWN` |
| D35 | Coupled lines CPL (P) | ng | `P ni1… gnd1 no1… gnd2 model [len=]`; node count by the model. On Xyce `P` is the port. | `dev::NGSPICE_OWN` (`Terminals::ByModel`) |
| D36 | HSPICE W element | hs | Multi-conductor lossy line on `RLGCmodel= RLGCfile= TABLEmodel= Umodel= FSmodel= Smodel=`; kept text; Spectre's `mtline` not mapped. | `Construct::HspiceWLine`; `rd/refused.rs::w_line` |
| D37 | XSPICE code model (A) | ng | `A… model`, node groups by the model; no reader rule (raw). Used by the Laplace lift (`s_xfer`) and by Verilog co-sim (`d_cosim`, `adc_bridge`, `dac_bridge`). | `dev::Primitive::CodeModel`; `src/python/verilog.rs` |
| D38 | LTspice A devices | lt | "Special functions" (gates, flip-flops, Schmitt, modulator, sample-and-hold, phase detector, varistor): private text. | `d/ltspice` kept_word `a` |
| D39 | OSDI instance (N) | ng | `N… nodes model` on a `.model` of a loaded module. | `dev::Primitive::Osdi`; `Construct::OsdiInstance` |
| D40 | Xyce `Y` devices | xy | `yneuron ysynapse yrom yrxn ygenext yacc ypde ypgbr ypgbs ypggb ypgtr ypowergridbranch ypowergridbusshunt ypowergridtransformer ypowergridgenbus ymemristor yadc ydac yext yibis yawl yneuronpop` + deprecated digital `ynot yinv ybuf yand ynand yor ynor yxor ynxor yadd ydff yjkff ytff ydltch`: private text. | `d/xyce` kept_word rows |
| D41 | Xyce `U` digital devices | xy | Every `U` line: private text. | `d/xyce` kept_word `u` |
| D42 | LTspice `.fra` devices | lt | `@` analyzer and `&` probe element lines: private text. | `d/ltspice` kept_word `@`, `&` |
| D43 | Semiconductor instance flags | ng hs xy | `off`, `on`, `thermal` end the node list before the model. | `dev::FLAGS` |
| D44 | Terminal count classes | — | Fixed(n), Range(lo,hi) (model decides), Any (X), ByModel (OSDI, XSPICE, CPL). | `dev::Terminals` |
| D45 | Model/value position classes | — | Model: None / AfterNodes / InArgs / Subckt; Value: None / First / Specs / Keys([…]). | `dev::{Model,Value}` |
| D46 | Spectre masters named directly | sp | `resistor capacitor inductor mutual_inductor vsource isource vcvs vccs cccs ccvs pvcvs pvccs pcccs pccvs bsource diode bjt mos1 mos2 mos3 bsim3v3 bsim4` (+ `iprobe port tline relay` in reader/writer). | `dev::SPECTRE` (22 rows) |
| D47 | VACASK modules | va | Built-ins `resistor capacitor inductor mutual vsource isource vcvs vccs cccs ccvs`; SPICE set `sp_resistor sp_capacitor sp_inductor sp_diode sp_bjt sp_mos1 sp_mos2 sp_mos3 sp_mos6 sp_mos9 sp_bsim3v3 sp_bsim4v8 sp_vdmos sp_jfet1 sp_mes1` (+ `jfet2`, `vbic13`, `vbic13_5t`, `psp103va`). An instance's master is always a model or a subckt. | `dev::VACASK` (25 rows); `d/vacask` model_master |
| D48 | Element card shape checks | — | A card that fails its letter's shape is kept raw; one with nothing after its name is unparsed; lines its own simulator rejects are dropped (P0010 Xyce, P0011 ngspice PWL `r=`). | `rd/elements.rs::place_element`; `rd/rejects.rs` |
| D49 | Node list punctuation | xy | Xyce reads `D1 (a 0) DM`, `V1 a,0`, `V1(a 0)`. | `rd/rejects.rs::words` |
| D50 | Element left as an error comment | cdl | An element the ngspice card has no form for is written as a comment naming its code (W0021). | `ws/cdl.rs` |
| D51 | Digital Verilog block | py | `verilog(...)` → XSPICE `d_cosim` instance (Icarus `.vvp`) or Yosys-synthesised standard cells. | `src/python/verilog.rs`; `src/veriloga.rs` |
| D52 | R/C/L value + model on one card | ng | `r1 1 0 100 rmodel l= w= m=2`: the value is the resistance, the model gives tc1/tc2, noise, defaults; `l`/`w` ignored when a value is given. | NG §2 row 13 |
| D53 | ngspice B-transformation of R/C/L | ng | A card whose text after the nodes holds `v(`, `i(`, `temper`, `hertz` or `time` (C: also `q=`) is rewritten: R → B, C → E+C+B, L → F+L+B; takes the first `{` on the line as the equation (model word or `l=`/`m=`/`tc1=` before `r=` gives a wrong value); drops the model card, `l w scale ac dtemp` and thermal noise. | NG §1 |
| D54 | Run-time expression with braces inside | ng | `R4 a b R=(time<12ms ? {Rload} : …)`: brace groups nested in a parenthesised expression. | NG §2 row 14 |
| D55 | Coupling with an expression k | ng hs xy lt sp va | `K12 L1 L2 {expr}`; today's view holds `k: f64`. | NG §2 row 11 |
| D56 | K with more than two inductors on ngspice | ng | ngspice expands `K1 L1 L2 L3 k` into every pair (reader refuses it today). | NG §2 row 12 |
| D57 | Switch initial state word | ng lt (`S1 … model ON/OFF`, `W1 … model off`) | Initial state for the operating-point Newton, not a forced state; Xyce has no ON/OFF; HSPICE `S` may be the S-parameter element; Spectre `relay` threshold mapping unsettled. | NG §2 row 17, §4 M5 |
| D58 | Source `DISTOF1`/`DISTOF2` | ng | Read only by `.disto`; inert without one. | NG §2 row 5 |
| D59 | Source `external` | ng | Value supplied by a shared-library callback: no other simulator has it. | NG §3 (4.1.9) |
| D60 | `VCVS`/`VCCS` keyword on E/G | ng hs | `E … VCVS a 0 2`: the keyword is dropped by ngspice; Xyce rejects it. | NG §2 row 28 |
| D61 | E/G `FREQ` source | ng lt (`Freq=`) | Frequency-table controlled source; HSPICE and Xyce lack it. | NG §3 (5.2.6) |
| D62 | E/G logic-gate forms | ng (`AND OR NAND NOR`), hs (`AND(k)` gate type) | PWL gate transfer; other targets need a B expression with breakpoints. | NG §3 (5.2.7) |
| D63 | B source temperature coefficients | ng | `b2 2 0 i=v(1) tc1= tc2= [temp= dtemp=]`: I(T)=I(TNOM)(1+TC1 ΔT+TC2 ΔT²); Xyce rejects `tc1` on B. | NG §2 row 18 |
| D64 | Diode positional area | ng | `D1 a 0 dmod 2`. | NG §2 row 23 |
| D65 | Diode thermal node | ng | `D1 1 0 tj dmod thermal` (`rth0`, `cth0`): a third node no other target's diode has. | NG §2 row 22 |
| D66 | Diode level 3 geometry | ng | Instance `w l` → area and perimeter; `m` applied twice (m² for m≠1). | NG §2 row X1 |
| D67 | HICUM node count | ng (3, 4 or 5 nodes), xy (always 5: substrate + thermal) | Missing substrate = ground, missing thermal = internal; the writer must pad. | NG §2 row X2 |
| D68 | MOS model families | ng | Levels 1, 2, 3, 6, 9; BSIM3 8/49; BSIM4 14/54; B4SOI 10/58; SOI 55–57; SOI3 60; HiSIM2 68; HiSIM-HV 73; binning; VDMOS (incl. `thermal`). | NG §3 (7.6, 7.7) |
| D69 | XSPICE A classes | ng | Analog, digital and hybrid code models; ngspice-private (should be PrivateText W0039, not W0001). | NG §3 (8), §4 M7 |
| D70 | Digital U gates | ng (PSpice-style `U … and(2)`), xy (own `U` devices) | Not probed. | NG §3 (10) |
| D71 | URC as a lumped ladder | ng | ngspice expands `U` into N lumps (R + C, or R + diode with `isperl`), geometric ratio k: lowerable to plain elements with fresh nodes. | NG §2 row 4 |
| D72 | LTRA / TXL / CPL model data | ng | `ltra r l g c len`; `txl r l g c length`; CPL R/L/G/C matrices; map to HSPICE `W`, Spectre `mtline`, Xyce `O` (single line only). | NG §2 rows 1–3 |
| D73 | VACASK ideal line | va | `tline_ideal.osdi` (lossless T). | NG §4 M2 |
| D74 | OSDI device with named ports | ng | IHP `Ncap PLUS MINUS cap_cmomf_mod w={w}` + `.model cap_cmomf_mod cap_cmomf`. | NG §2 row 31 |
| D75 | LTspice current-source `load` flag | lt | `I … load`: I = I0·g(V), g = 4V (V<0), 4V(1−V) (0≤V<0.5), 1 (V≥0.5); multiplies a waveform too; negative I0 with `load` gives 0 A (measured, contradicts the help). | LH L7 |
| D76 | LTspice current-source table / step / R forms | lt | `I tbl=(v,i,…)` / `table=` (x is V(n+,n-), ends held); `I <v> step(v1,v2,…)` (used with `.tran … step`); `I n+ n- R=<v>` (a resistor). | LH L8–L10 |
| D77 | LTspice A device kinds | lt | AND, OR, XOR, BUF, INV, SCHMITT, DFLOP, VARISTOR, MODULATOR, SAMPLEHOLD, OTA …; grounding an input removes it from a gate. | LH L11 |
| D78 | Linear E/G with an expression gain | lt ng | `G3 0 o3 a 0 {2/R1}`: gain is an expression, not a number (today's view holds `gain: f64`). | LH L12 |
| D79 | LTspice hysteretic-core inductor | lt | `Hc= Bs= Br= A= Lm= Lg= N=` (Chan model): no other target. | LH L16 |
| D80 | Flux-defined inductor | lt | `L … Flux=<expr of x>` (x = current); B source `V=ddt(flux(i))` elsewhere. | LH L17 |
| D81 | `R=<expr>` keyword resistor with `temp` | lt | `R6 a 0 R={…}`; `temp` in the expression is the circuit temperature. | LH L18 |
| D82 | Resistor `noiseless` flag | lt | ngspice `noisy=0`, Spectre `isnoisy=no`, VACASK model `has_noise=0`, Xyce none. | LH L19 |
| D83 | LTspice B-source forms | lt | `B I=… Rpar=`, `B R=<expr>`, `B P=<expr>` (power), `B … laplace=`. | LH L33 |
| D84 | T line `Zo=`/`Td=` spelling | lt | Alias of `Z0=`/`TD=`. | LH L35 |
| D85 | LTspice implicit inductor Rser | lt | An inductor not in a `K` has an implicit Rser = 1 mΩ (none when coupled, none with `Rser=0`); no hidden damping. | LH L38 |
| D86 | LTspice implicit capacitor Cpar | lt | A capacitor with `Lser>0` and no `Cpar` carries C·1e-6 across its terminals (×m); `Cpar=0` removes it. | LH L39 |
| D87 | LTspice capacitor `RLshunt` | lt | A resistor across the `Lser` element only. | LH L3 |
| D88 | `.nodeset` / `.ic` inside a subcircuit | lt | Node values scoped to a subckt: need hierarchical names elsewhere. | LH L37 |
| D89 | HSPICE E/G/H `value={…}` with braces | hs (ESPice reading) | HSPICE manuals spell expressions in single quotes; ESPice also reads `{…}` in the value slot (braces stay name bytes elsewhere). | LH H1, H7 |
| D90 | HSPICE E `DELAY` | hs | `E … DELAY in+ in- TD=`: a pure delay; ngspice via an ideal T line, Xyce `YDELAY`, LTspice `delay(x,t)`. | LH H3 |
| D91 | HSPICE E/G `LAPLACE` / `POLE` | hs | `LAPLACE in+ in- n0 n1 … / d0 d1 …` (ascending coefficients) and `POLE a z1r,z1i … / b p1r,p1i …` (roots α + j2πf); Spectre `svcvs`/`svccs` with poles/zeros. | LH H4 |
| D92 | HSPICE G table with smoothing | hs | `G table {expr} = (x,y)…`, corners smoothed over `DELTA` (default a quarter of the smallest breakpoint gap). | LH H5 |
| D93 | HSPICE `VCR` / `VCCAP` | hs | Voltage-controlled resistor / capacitor on a G element; VCCAP's charge definition unsettled. | LH H6 |
| D94 | HSPICE E `OPAMP`, `TRANSFORMER` | hs | Ideal op-amp; ideal transformer (E + F pair elsewhere). | LH H8 |
| D95 | HSPICE W element model | hs | `.model … W MODELTYPE=RLGC N= Lo= Co= Ro= Go= Rs= Gd=`; constant RLGC maps to TXL/LTRA/CPL/`tline`/`mtline`; `Rs`/`Gd` (skin, dielectric) only Spectre `mtline`. | LH H9 |
| D96 | HSPICE S element | hs | `S … mname=` with `.model … S TSTONEFILE=`: Touchstone N-port; Xyce `YLIN`, Spectre `nport file=`. | LH H10 |
| D97 | HSPICE P port | hs | `P n+ n- port=n z0= [dc=] [ac/tran]`, `hblin=[…]`. | LH H11 |
| D98 | HSPICE B element | hs | Real HSPICE `B` is an IBIS buffer; ESPice's corpus uses `b… v=expr` as a behavioural source. | LH H42 |
| D99 | HSPICE C/L `POLY` | hs | `C1 a 0 POLY c0 c1 …`: polynomial capacitance C(v) / inductance. | LH H33 |
| D100 | `.connect a b` | hs | Two nets are one: rename in scope or a 0 V source. | LH H22 |
| D101 | LTspice B extras | lt | `ic`, `tripdv`/`tripdt` (step rejection), `Rpar`. | LH §5 |
| D102 | LTspice E/G `table=` | lt | Table transfer on E/G (kept, W0032 today). | LH §5 |
| D103 | IGBT | lt (`Z` with `NIGBT`/`PIGBT` models) | LTspice's `Z` is MESFET or IGBT by model. | LH §5 |
| D104 | HSPICE R/C/L equation values | hs | `R='eq'`, `C='eq'` with `TC1 TC2 SCALE M DTEMP`. | LH §6 |
| D105 | HSPICE magnetic winding / core | hs | Magnetic winding element + mutual core model: no other target (Xyce nonlinear K unsettled). | LH §6 |
| D106 | HSPICE controlled-source forms | hs | E: `PWL(1)`, gates, `DELAY`, `VOL=`/`VALUE=`, `OPAMP`, `TRANSFORMER`, `LAPLACE`, `POLE`, modifiers `MAX MIN SCALE TC ABS IC`; F/H: gain, `POLY`, `PWL`, `DELAY`, `VALUE`; G: `POLY`, `PWL`, `VCR`, `VCCAP`, `CUR=`, `TABLE`, `LAPLACE`, `POLE`. | LH §6 |
| D107 | HSPICE U elements | hs | `U` lossy transmission line (RLGC U model) and `U` digital input element (stimuli). | LH §6 |
| D108 | HSPICE op-amp element | hs | A built-in op-amp element with its own model. | LH §6 |
| D109 | `M=` on an independent source | xy | Xyce ignores `M` on V/I sources (no such parameter). | XY S1 |
| D110 | Xyce source oddities | xy | `Vsrc 1 0 1 0` (DC 1), `DC pulse((…))` (double parentheses), a waveform with no closing `)`, unit letters after numbers ignored. | XY S3, S7 |
| D111 | Xyce port with a source | xy | `P1 1 0 port=2 sin 0 1 1e5`, `P1 1 0 port=1 z0=50 AC 1`; a `P` line with no `port=`. | XY S9 |
| D112 | Xyce R/C/L instance extras | xy | `TC=a,b` (TC1, TC2), `TEMP=`, `DTEMP=`, `TCE=`, capacitor `AGE=`/`D=`; R level 2 (thermal). | XY E4, E5, §2b |
| D113 | Positional area on D and J | xy | `D2 15 17 SWITCH 1.5`, `J13 22 14 23 JNOM 2.0`. | XY E6 |
| D114 | Core-model coupling details | xy | `K1 L1 L2 1 nlcore`: Jiles-Atherton `CORE` model; the L values become turns. | XY E7 |
| D115 | Coupling keyword / time-dependent k | xy | `K2 L1 L2 K = 0.5`, `k={expr of time}`. | XY E8 |
| D116 | E/G `VALUE` without `=` or braces | xy | `E1 5 0 VALUE {2*V(1)}`, `E2 6 0 VALUE 3*V(1)`; `smoothbsrc=1`. | XY E9 |
| D117 | Xyce switch law | xy | Resistance moves smoothly from ROFF to RON (exponential blend of the control); models `VSWITCH`/`ISWITCH`; `ON`/`OFF` is only a first guess. | XY E12, E13 |
| D118 | Generic expression-controlled switch | xy | `S… n+ n- <model> [ON/OFF] CONTROL={expr}`. | XY E14 |
| D119 | BJT with exposed internal nodes | xy | `Q c b e s cprime bprime eprime model` (7 nodes, undocumented). | XY E15 |
| D120 | Xyce device levels | xy | D 1, 2, 200 (JUNCAP); J 1, 2; M 1, 2, 3, 6, 9, 10, 14/54, 18, 77, 102, 103, 107, 110, 301, 2000, 2001; Q 1, 11, 12, 23, 230, 234, 504, 505; R 1, 2; B with `table`/`tablefile`. | XY §2b |
| D121 | Verilog-A plugin instances on Xyce | xy | `Yrlc2 rlc1 …`, `YVSRC VCC 1 0 Voltage=5V`: a module loaded with `Xyce -plugin`. | XY Y5 |
| D122 | Xyce `YLIN`, `YTRANSLINE`, `YDELAY` | xy | Touchstone multiport (HB only); lumped line (`len`, `lumps`, per-length r l c); ideal delayed VCVS (`TD`, `LINEARINTERP`, `BPENABLED`, `EXTRAPOLATION`). | XY Y6–Y8 |
| D123 | Xyce digital gate shape | xy | `UAND1 AND(4) dig_pn 0 … DMOD`: behavioural gates with an analog interface (switched resistors, timing in the model). | XY Y9 |
| D124 | VACASK behavioural source keys | va | `b (p n) v=` / `i=` / `potential=` / `flow=`. | VS §2 Statements |
| D125 | VACASK PWL source keys | va | `wave=`, `period`, `scale`, `offset`, `stretch`, `file`. | VS §2 Devices |
| D126 | VACASK VDMOS thermal nodes | va | `m1 (d g 0) nm $mfactor=2`; thermal form `(d g 0 tj tc)`. | VS V30 |
| D127 | `$mfactor` on sources | va | On vsource, isource, vccs, vcvs, cccs, ccvs. | VS V31 |
| D128 | Other VACASK OSDI modules | va | `psp103v4`, `bsimbulk106`, `vbic`, `tline_ideal`, `opamp` read as OSDI instances. | VS §2 Devices |
| D129 | Spectre source extras | sp | `ampl2`/`freq2` (second sinusoid), `ammodindex`/`ammodfreq`/`ammodphase`, `pacmag`/`pacphase`/`xfmag` (per-analysis magnitudes), `tc1`/`tc2` on a source, `m=` on V/I, `noisevec`/`noisefile`, `pwl file=`. | VS §2b Components |
| D130 | Spectre bsource extras | sp | `q=`, `r=`/`g=`, `c=` outputs; `ddt()` and `idt()` inside the expression. | VS §2b Components |
| D131 | Spectre components with no card today | sp | `transformer`, `switch` (fixed position), `relay`, `delay`, `nport`, s-domain sources, z-domain sources, `core`/`winding`, `msline`, `cktrom`, `a2d`/`d2a`, `fourier`, `ratiometric`, `node`, `quantity`, `analogmodel`. | VS §2b Components |
| D132 | Spectre device families with no master row | sp | `bsim1 bsim2 bsim3 bsimsoi b3soipd vbic` MEXTRAM, `jfet`, `gaas`, `mos0`, `ekv`, `hisim`, Philips/UCSD/TFT families. | VS §2b Components |
| D133 | Master in an unread include | sp va | An instance whose master is declared in a file the kit did not read: its kind is unknown at read time. | VS V29 |
| D134 | Designed line record | ng hs xy lt sp va | One `Line{name, n conductors (≤ 255; CPL cap 8), nodes [in_1..in_n, in_ref, out_1..out_n, out_ref], len, rlgc (four lower-triangular per-unit-length matrices), dispersive (HSPICE Rs/Gd present), source: Ltra/Txl/Cpl/HspiceW/Mtline/Tline/TlineIdeal, model}`; derived loss case LC/RLC/RC/RG/RLGC, grounded, lossless. | D-lines §2 |
| D135 | Generic PDK-neutral devices | py (planned) | Kinds `nmos pmos res cap ind bjt diode`; open `variant`/`flavour`; physical params `w l nf m value area polarity` in SI; an element whose master is the reserved `gen__<kind>[__<flavour>]` (a user name with that prefix: P0050). | D-pdk-binding §2 |
| D136 | Device terminal names | — | Canonical terminal names by primitive: R/C/L `(1, 2)`; D `(a, k)`; Q `(c, b, e, s)`; M `(d, g, s, b, …)`; J/Z `(d, g, s)`; ≤ 7 terminals (BSIMSOI). | D-probe-terminals §2 |
<!-- D-end -->

## E. Source waveforms

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| E1 | DC value | all | Leading value, `dc v`, `dc=v`; Spectre `dc=`; VACASK `dc=` is `type="dc"`'s value, a waveform's DC is its value at t=0. | `rd/sources.rs`; `rd/named_elements.rs` |
| E2 | AC small-signal | all | `ac [mag [phase]]`, `ac=mag`; Spectre/VACASK `mag=`, `phase=`; magnitude default 1. | `Card::Source::{ac_mag,ac_phase}` |
| E3 | PULSE | ng es hs xy lt; sp va (`type=pulse val0 val1 delay rise fall width period`) | 7 values V1 V2 TD TR TF PW PER + 8th cycle count (ngspice NP, LTspice Ncycles; Xyce ignores it; elsewhere cut W0026 or expanded to PWL, ≤ 1000 pulses). | `WaveKind::Pulse` (held 8); `rd/wave_rules.rs::{counted,pulses}` |
| E4 | SIN / SINE | ng es hs xy lt; sp va (`type=sine sinedc ampl freq delay damp/theta sinephase`) | 6 values VO VA FREQ TD THETA PHASE + 7th LTspice Ncycles (written elsewhere as an expression of time). | `WaveKind::Sin` (held 7); `rd/wave_rules.rs::{cycled,sin_cycles}` |
| E5 | EXP | ng es hs xy lt; sp (`type=exp val0 val1 td1 tau1 td2 tau2`); va (`delay tau1 td2 tau2`, td2 relative to delay) | 6 values V1 V2 TD1 TAU1 TD2 TAU2. | `WaveKind::Exp` (held 6); `rd/named_waves.rs` |
| E6 | PWL | ng es hs xy lt; sp (`type=pwl wave=[t v …]`); va (`type="pwl"`) | Flat time/value pairs (numbers or expressions). | `WaveKind::Pwl` |
| E7 | PWL repeat / delay | ng (`r=`, `td=`, also inside the parentheses; V only), es (V and I), sp (`pwlperiod=`, `delay=`) | `r` must be a time point before the last (P0011); Spectre form only when the wave repeats exactly; others W0008. | `Wave::{repeat,delay}`; `Construct::{PwlRepeatV,PwlRepeatI}`; `Alt::PwlPeriod` |
| E8 | PWL relative times | lt | A time written `+t` or `+ t` counts from the previous point; with expressions becomes `prev+(t)`. | `rd/wave_rules.rs::pwl_relative` |
| E9 | PWL from file | ng? xy? | Only seen as `file` in the reject scanner (points in another file). | `rd/rejects.rs::Pwl::file` |
| E10 | SFFM | ng (VO VA FM MDI FC, 0 until t=0, MDI limited, FC=0 → 500/TSTOP), hs xy lt (SPICE3: VO VA FC MDI FS), sp (`type=sine fmmodindex fmmodfreq`), va (`type="fm" modindex modfreq`) | Two argument orders and two value laws; convertible only under conditions, else written as an expression of time. | `WaveKind::Sffm` (held 5); `rd/wave_rules.rs::{ngspice_sffm,spice3_sffm,sffm_of}`; `beh::NetlistRules` |
| E11 | AM | ng (`AM(VO VMO VMA FM FC TD PHASEM PHASEC)`), hs (`AM(SA OC FM FC TD)`), va (`type="am" sinedc ampl modindex modfreq freq delay modphase tdphase`), es (no VO offset) | Held in ngspice-45 order (8 values); Xyce/LTspice/Spectre W0108. | `WaveKind::Am`; `Construct::AmWave`; `Alt::{HspiceAm,AmNoOffset}`; `rd/sources.rs::am` |
| E12 | TRNOISE | ng | Up to 7 values; adds to the DC value; others W0009. | `WaveKind::Trnoise`; `Construct::NoiseWave` |
| E13 | TRRANDOM | ng | Type 1–4 (uniform, gaussian, exponential, poisson) + 4 values (`p1` default 1); adds to DC. | `WaveKind::Trrandom`; `src/python/keywords.rs::RandomKind` |
| E14 | PAT | xy (hs?) | Bit-pattern source (VHI VLO TD TR TF TSAMPLE DATA `b…`, `RB=`): only in the Xyce reject scanner today. | `rd/rejects.rs::pat_rejected` |
| E15 | Waveform with/without parentheses | ng es hs xy lt | `PULSE(0 1 …)` or `PULSE 0 1 …`; commas allowed among values. | `rd/sources.rs::spec` |
| E16 | Waveform values as expressions | ng xy lt sp va | Each value a number or an expression. | `rd/sources.rs::Num` |
| E17 | Extra values cut | — | Values past what the target's wave holds are not written (W0026). | `Wave::cut` |
| E18 | Start-up ramp | lt (`.tran … startup`) | DC-only top sources ramp `PWL(0 0 20u X)`; elsewhere a lift. | `cap::Rule::StartupRamp`; `rd/sources.rs::ramp_startup` |
| E19 | Spectre source keys | sp | `dc mag phase type val0 val1 delay rise fall width period sinedc ampl freq damp sinephase fmmodindex fmmodfreq td1 tau1 td2 tau2 wave pwlperiod`. | `rd/named_elements.rs::SPECTRE` |
| E20 | VACASK source keys | va | `dc mag phase type val0 val1 delay rise fall width period sinedc ampl freq theta modindex modfreq modphase tdphase tau1 td2 tau2 wave`. | `rd/named_elements.rs::VACASK` |
| E21 | SFFM with delay and phases | ng | `SFFM(VO VA FM MDI FC TD PHM PHC)` (7–8 values); kept text today. | NG §2 row 8 |
| E22 | PWL from a file, per dialect | ng (`file=`), xy (`PWL FILE "f"`, with `TD=`/`R=`), lt (`PWL file=`), va (`type="pwl" file=`), sp (`pwl file=`) | File path + optional repeat/delay. | NG §3 (4.1.4) |
| E23 | PWL points as quoted expressions on continuation lines | ng | `v1 in gnd pwl` / `+ t 'expr'` pairs. | NG §2 row 9 |
| E24 | RF port power/frequency | ng | `portnum n z0 Z [pwr freq]`. | NG §2 row 6 |
| E25 | AM/SFFM value before the delay | ng | 0 for t ≤ TD (also at the operating point), not VO: only a B-form is exact when VO ≠ 0. | NG §2 rows 7, 8, §5.3 |
| E26 | AC magnitude as an expression | lt ng | `V … AC {1-N}` (today `ac_mag: Option<f64>`). | LH L20 |
| E27 | Wave file source | lt | `V/I … wavefile=<f> [chan=]` (.wav, full scale ±1): no other target; could be decoded to a PWL at import. | LH L21 |
| E28 | LTspice PWL extras | lt | `Trigger=<expr>`, `REPEAT FOR n / FOREVER … ENDREPEAT` blocks, `TIME_SCALE_FACTOR=`, `VALUE_SCALE_FACTOR=`, `file=`. | LH L34 |
| E29 | HSPICE PWL `R`/`TD` inside the parentheses | hs | `pwl(0 0 1n 1 r=0 td=1n)`; `R` alone repeats from 0. | LH H30 |
| E30 | HSPICE PWL with no t=0 point | hs | Uses the source's DC value at time zero (ngspice holds the first value). | LH H31 |
| E31 | HSPICE PWL `PL` | hs | Value-time (reversed) pairs. | LH H32 |
| E32 | Pattern source | hs (`PAT (vhi vlo td tr tf tsample b-strings r= rb=)`, `.pat`), xy (`PAT(VHI VLO TD TR TF TSAMPLE DATA R RB)`) | Bit strings (`b1011`, `m` = midpoint), repeat counts; Xyce repeat semantics differ (unsettled): lower to a finite PWL. | LH H24 |
| E33 | Digital vector file | hs | `.vec 'file'`: stimuli (→ PWL) plus output checks (`dout`). | LH H25 |
| E34 | HSPICE data-driven PWL | hs | A PWL whose points come from a `.DATA` table. | LH §6 |
| E35 | Xyce PWL repeat law | xy | `PWL(…) R=0.5 TD=1`: repeat from time R to the last point; R may fall between points (unlike ngspice, where it must be a point). | XY S5 |
| E36 | Waveform arguments braced/quoted without parentheses | xy | `pulse 1V 2V 0 1e-5 1e-5 {…} {…}`, `SIN {0.5*Vmag} {0.5*Vmag} 20MEG`, `pwl 0ns 'vp' …`; commas allowed. | XY S3 |
| E37 | AC phase as an expression | xy | `AC {expr} [{expr}]`. | XY S2 |
| E38 | VACASK phase keys | va | AM `phase`, `tdphase` and its DC value; `tdphase` on sine/fm/pwl; every waveform's DC value is the waveform at t=0. | VS V32–V34 |
<!-- E-end -->

## F. Models

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| F1 | Model card | ng es hs xy lt (`.model name type (k=v …)`), sp (`model name master type=… k=v`), va (`model name module k=v`) | Name, kind word, level, polarity, parameter list (flags, sequences, strings); parentheses and commas are separators only. | `ast::Tag::Model`; `rd/cards.rs::Model{name,kind,params}`; `res::DefKind::Model{word,kind,level,polarity}` |
| F2 | SPICE model types | ng… | `nmos pmos npn pnp d njf pjf nmf pmf r c l res cap ind sw csw vdmos ltra txl cpl urc nhfet phfet` (+ Xyce `core`). | `d/spectre`, `d/vacask` `spice_model_type` rows |
| F3 | Level numbering | — | Neutral = ngspice's. Aliases: HSPICE BJT 11 (UCSD HBT) has no ngspice level; Xyce JFET 2 (Shockley + THETA/DELTA) ≠ ngspice 2 (Parker-Skellern); Xyce BSIM3 8/9 read as 49; Xyce PSP 103 = ESPice 1040; Xyce VBIC 11/12; ngspice level > 99 silently becomes 1. | `beh::LevelAlias`; `Behaviours::{neutral_level,same_level}`; `rd/cards.rs::model_view`; `ws/ngspice_models.rs`; W0104, W0111 |
| F4 | Level → master (Spectre) | sp | nmos/pmos 1→`mos1`, 2→`mos2`, 3→`mos3`, 8/49→`bsim3v3`, 14/54→`bsim4` (`type=n/p`); npn/pnp 1→`bjt` (`type=npn/pnp`); d 1→`diode`. Reverse: `spectre_kind`. | `d/spectre` model_master (13 rows); `rd/named.rs::spectre_kind` |
| F5 | Level → module (VACASK) | va | MOS 1,2,3,6,9→`mos1…mos9`; 8/49→`bsim3v3`; 14/54→`bsim4v8`; 103/1040→`psp103va`; d→`diode`; BJT 1→`bjt`, 11→`vbic13`, 12→`vbic13_5t`; JFET 1→`jfet1`, 2→`jfet2`; MES 1→`mes1`; `vdmos` (`type=±1`); written `sp_<module>`; file per module (`model_load`). | `d/vacask` model_master (30 rows), model_load (3 rows); `rd/named.rs::vacask_kind` |
| F6 | Model polarity | all | From the type word (`nmos`, `pnp`) or `type=` (Spectre `n`/`p`/`npn`/`pnp`, VACASK `1`/`-1`). | `res::Polarity{N,P}`; `beh::ModelMaster::type_` |
| F7 | Binned models | ng hs (`name.1`, `name.2` with `lmin lmax wmin wmax`), sp (model group `model n master { 1: … 2: … }`) | A base name + bins; a run of SPICE bins is written as one Spectre group. | `res::Def::base`; `Block::models` (statement, bin); `rd/named_blocks.rs::named_model_ok`; `wn/cards.rs` |
| F8 | `ako:` derived model | lt (PSpice form; ngspice in PSPICE mode) | `.model new ako:base [type] (k=v …)`: base's parameters with overrides, chains allowed; unresolved → W0142. | `rd/cards.rs::derived` |
| F9 | R/C/L models | ng (semantic source) | An R/C/L card naming a model: mapped only when read as ngspice; Xyce's R/C/L model parameters are multipliers (W0051); LTspice/Spectre/VACASK refuse an unmapped one (W0006). | `Construct::RclModel`; `rd/elements.rs::same_rcl_model`; `ws/values.rs::r_value` |
| F10 | ngspice resistor model params | ng | `rsh narrow dw short dlr tc1 tc1r tc2 tc2r tce defw w l kf af lf wf ef tnom r res bv_max` + instance defaults; Spectre names (`etch etchl leexp weexp fexp …`), implied defaults (`l w r af wdexp ldexp leexp weexp`), exclusive pair (`r`,`rsh`). | `r/models.rs::RESISTOR` |
| F11 | ngspice capacitor model params | ng | `cap cj cox cjsw capsw defw w defl l narrow short del tc1 tc2 tnom di thick bv_max c` …; implied `w=1e-05`. | `r/models.rs::CAPACITOR` |
| F12 | ngspice diode level 3 params | ng | ~100 names with aliases (`is/js`, `cjo/cj0/cj`, `vj/pb`, `bv/vb/vrb/var` …); Spectre laws rewritten (`jsw/ns`, `tcv`→`tbv1`), implied `eg nz fcs xom xoi dskip jmax`. | `r/models.rs::DIODE3`; `wn/params.rs` |
| F13 | Parameter answer classes | — | To(name) / Same / Ignored / Off(v) / Knee / YesNo / Flag / TimesM / OverM / Refuse; Implied Text / Copy / Unless / Eg. | `r/models.rs::{Param,Implied,Kind}` |
| F14 | Per-device parameter name tables | ng va xy | Generated lists: ngspice (bjt bsim3v3 bsim4v8 diode hicum jfet1 jfet2 mes1 mos1 mos2 mos3 mos6 mos9 vdmos), VACASK (same + vbic13, vbic13_5t, psp103va, `.inst` rows), Xyce (diode bjt hicum mes1). A name the target device lacks is a silent wrong number (W0050, W0129, W0114) or dropped (W0112). | `wn/ngspice_params.txt` (15 lines); `wn/vacask_params.txt` (22 lines); `ws/xyce_params.txt` (5 lines) |
| F15 | VACASK renamed parameters | va | Model parameters clashing with an instance parameter are `model_<p>`; instance ones `instance_<p>`. | `r/models.rs` rows; `rd/named_elements.rs` |
| F16 | Model value forms | — | Number, expression, string (`version="4.8"`), VACASK parenthesised number `binunit=(1)`, matrix sequence. | `rd/named_blocks.rs::model_value`; `rd/cards.rs::model_head` |
| F17 | ngspice built-in device names | ng | 57 names (`BSIM4`, `VBIC`, `Mos1`, `LTRA`, `NUMD` …): a model type of one of these is the built-in, never a loaded OSDI module (W0121). | `d/ngspice` builtin_device rows |
| F18 | Model master of the target's own | sp va | A type that is no SPICE type is written as it is (Spectre `bsim4`, a loaded VACASK module `osdi:<module>`). | `beh::MasterAnswer{Master,Refused,AsWritten}`; `rd/named.rs::vacask_model_kind` |
| F19 | Xyce model cards | xy | Levels with no Xyce device left out (W0053) and refused at the instance (W0050); diode knee current laws differ (PSpice vs SPICE, 22 % at 0.75 V). | `ws/xyce_models.rs`; `ws/ngspice_models.rs` |
| F20 | XSPICE model for a lift | ng | `.model … s_xfer` written by the Laplace lift. | `cap::Rule::LaplaceXfer` |
| F21 | Model in which file | all | The card an instance uses may sit in another kit file; judged at the instance. | `rd/model_cards.rs::{instance_model,file_models}` |
| F22 | LTspice model types | lt | `D NPN PNP NJF PJF NMOS PMOS NMF PMF SW CSW VDMOS NIGBT PIGBT URC LTRA`; diode model ratings `Vpk Ipk Iave Irms diss` have no simulation meaning. | LH §5 |
| F23 | Model alias | hs | `.MALIAS` / `.ALIAS`: another name for a model. | LH §6 |
| F24 | `.DEL LIB` | hs | Removes a library section in an `.alter` run. | LH §6 |
<!-- F-end -->

## G. Analyses

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| G1 | Operating point | ng es hs xy lt (`.op`), sp (`dc` with no parameters), va (`analysis n op`) | No parameters. HSPICE `.OP format time …` is a timed report (kept text). On Xyce an `.op` beside another analysis writes nothing (W0052). | `stmt::Analysis::Op`; `rd/testbench.rs::Ana::Op` |
| G2 | DC sweep, linear | ng es hs xy lt (`.dc src start stop step [src2 …]`), sp (`dc dev= param=dc start= stop= step=` or `param=`), va (`sweep … ` before `analysis n op`) | Ordered axes (first = innermost on SPICE); each: swept name, start, stop, step. HSPICE `var START= STOP= STEP=`. | `Ana::Dc(Vec<(src,start,stop,step)>)` |
| G3 | DC sweep, log / list axis | xy (`DEC/OCT/LIN src start stop n`, `src LIST v…`, `LIST src v…`), lt (`.dc dec src …`), hs (`var DEC/OCT/LIN np start stop`, `var POI np v…`), sp (`values=[…]`), va (`mode="dec"/"oct" points=`, `values=[…]`) | Per axis: Lin / Log(dec or oct, points per) / List. ngspice writes `foreach` loops; ESPice W0038. | `rd/sweeps.rs::{DcAxis,DcSweep{Lin,Log,List}}`; `Construct::DcLogList`; `Alt::{DcType,ControlScript}` |
| G4 | What a sweep names | — | Temperature, a top parameter, a source's DC value, an R/C/L value, another device or a device parameter (Xyce: a bare device name is its default parameter; a parameter of that name wins). LTspice `.dc` sweeps a source or `temp` only. | `r/swept.rs::Swept` |
| G5 | AC | ng es hs xy lt (`.ac dec/oct/lin n f1 f2`), sp (`ac start= stop= dec=/lin=/log=` or `center= span=`), va (`ac from= to= mode= points=` or `values=[f]`) | Variation, points, start, stop. Spectre `lin=`/`log=` and VACASK `points` (lin) count steps (one fewer than points). Spectre AC over `dev=/mod=/param=` at one `freq=` stays text. | `Ana::Ac(Sweep{variation,points,start,stop})` |
| G6 | Transient | all | `.tran tstep tstop [tstart [tmax]] [uic]`; lt: `.tran tstop` alone, `startup`, `uic`; xy: `NOOP` = `UIC`, step 0 = simulator's choice; hs: `.tran s1 t1 s2 t2 … [START=]`; sp: `tran step= stop= start= maxstep= skipdc=yes`; va: `tran step= stop= maxstep= icmode="uic" ic=[…] nodeset=[…]`. | `Ana::Tran{step,stop,start,max_step,uic,startup}`; `ws/tstep.rs`; W0141 |
| G7 | Transient max-step option | hs (`.option delmax=`) | A deck-wide cap applied to every transient. | `rd/options.rs::Opt::MaxStep`; `Ana::cap_step` |
| G8 | Noise | ng es xy lt (`.noise v(out[,ref]) src dec n f1 f2 [pts_per_summary]`), hs (`.noise out src [interval]` over the deck's `.ac`), sp (`n (o r) noise iprobe= …`), va (`noise out=["o","r"] in="src" …`) | Output node pair, input source, sweep, summary interval (W0033 when dropped). Xyce writes integrated noise to stdout only (W0052). | `Ana::Noise{output,reference,source,sweep,pps}` |
| G9 | DC transfer function | ng es hs lt (`.tf out src`), sp (`xf`), va (`dcxf out=`) | Output (voltage or `i(src)`), input source. Xyce W0015. Input/output resistance not computed on some targets (W0125). | `Ana::Tf(out,src)`; `Construct::Tf` |
| G10 | DC sensitivity | ng es hs (`.sens out`), sp (`sens`) | Output only. Xyce, LTspice, VACASK W0015. | `Ana::Sens(out,None)`; `Construct::Sens` |
| G11 | AC sensitivity | ng es (`.sens out ac dec n f1 f2`), sp | Output + sweep. | `Ana::Sens(out,Some(sweep))`; `Construct::SensAc` |
| G12 | Xyce `.SENS` | xy | Objective(s) and parameter lists over the deck's `.dc`/`.tran`/`.ac`: kept text. | `Construct::XyceSens` |
| G13 | Pole-zero | ng es (`.pz in+ in- out+ out- vol/cur pol/zer/pz`), hs (`.pz v(out[,ref]) src`), sp (`pz (o1 o2) iprobe=src`) | Four nodes, transfer type, what to find. | `Ana::Pz([String;4],tf,kind)`; `Alt::HspicePz` |
| G14 | Fourier | ng es hs xy lt (`.four f out…`; alias `.fourier` except on LTspice where `.fourier` is its own card), sp (`fourier` component) | Fundamental + outputs; harmonic count: ngspice `.options nfreqs=`, LTspice on the card, Spectre parameters. Result is a table, not a plot. | `Ana::Four(f,outs)`; `stmt::Analysis::{Four,Fourier}`; `src/python/analysis_region.rs` |
| G15 | S-parameters | ng (`.sp dec n f1 f2`), xy (`.LIN [sparcalc=…]` beside `.AC`), sp (`sp` with ports), va (`acsp`, native cell) | Sweep over all ports. HSPICE `.lin` not written (W0015). | `Ana::Sp(Sweep)`; `Construct::SParameter`; `Alt::XyceLin`; `beh::DotRule::SParams` |
| G16 | Harmonic balance, one tone | xy (`.HB f` + `.OPTIONS HBINT NUMFREQ=k`, default 10), hs (`.HB TONES=f NHARMS=k`), es (`.hb f k` / `tones= nharms=`), sp (`hb fundfreqs=[f] maxharms=[k]`), va (`analysis n hb freq=[f] nharm=k`) | Tone + harmonics above DC; two-tone, non-integer counts and Xyce `HBOSC` stay text; phasor convention differs (Xyce two-sided). ngspice, LTspice W0015. | `Ana::Hb{tone,harms}`; `Construct::HarmonicBalance`; `beh::DotRule::Hb`; `rd/hb.rs` |
| G17 | Monte Carlo analysis | sp (`mc montecarlo numruns= seed= variations=process/mismatch/all { inner }`), va (`mc` loop) | Runs, seed, variations, one inner analysis. HSPICE `sweep monte=`, Xyce `.sampling` named but not written. VACASK varies wider (W0115). | `Ana::MonteCarlo{name,runs,seed,variations,inner}`; `stmt::Analysis::Montecarlo` |
| G18 | Any other Spectre analysis | sp | Kept as the statement: `acmatch dcmatch hbnoise qpnoise pdisto pnoise hbstb envlp hbsp hbac hbxf qpac qpsp qpss qpxf sweep pac psp pss pstb pxf tdr stb xf sens`. | `Ana::Spectre{kind,node}`; `d/spectre` MASTERS (34 analysis rows); `rd/named_bench.rs::spectre_refusal` |
| G19 | VACASK-only analyses | va | `dcinc` (small-signal DC), `dcxf`, `acxf`, `acstb`, `acsp`, `hb`, `hbac`, `hbnoise`, `pss`, `pac`, `pnoise`, `trannoise`, `mc`. | `stmt::Analysis::{Dcinc,Dcxf,Acxf}`; `d/vacask` ANALYSES (8), vacask_analysis (10); `rd/named_bench.rs::vacask_refusal` |
| G20 | HSPICE-only analyses | hs | `.sn .snac .snnoise .snxf .ptdnoise .sample .trannoise .lstb`, timed `.op`: kept text. | `Construct::HspiceAnalysis`; `d/hspice` kept_word |
| G21 | Xyce-only analyses | xy | `.mpde .mor .pce .embeddedsampling .sampling .result .rol .rol_dc .rol_obj`: private text. | `d/xyce` kept_word; `rd/refused.rs::dialect_raw` |
| G22 | LTspice `.fra` | lt | Time-domain frequency response analysis: private text. | `d/ltspice` kept_word |
| G23 | ngspice `.pss`, `.disto` | ng | `.pss` kept as ngspice text (Spectre cannot run it; Python writes the Spectre `pss` statement on Spectre); `disto` is a backend kind and a raw dot word. | `src/python/analysis.rs`; `src/backend/kind.rs::Kind::{Pss,Disto}`; `RAWKIND` |
| G24 | Parameter step, range | xy lt (`.step [lin/dec/oct] name start stop step`; lt `.step param name …`), sp (`sweep param= start= stop= step=/lin= { … }`), va (`sweep name … from= to= step=/mode= points=`), py (adds `log` = N points in all) | Name, grid kind, three numbers; first `.step` is the innermost loop; LTspice sorts bounds and always ends on the stop value (a grid that misses it becomes a list); cap 4096 points. ngspice: `foreach` loop; HSPICE/ESPice W0016. | `rd/testbench.rs::BenchCard::Step{sweep,param,nums}`; `rd/sweeps.rs::{step_points,ltspice_step,STEP_POINTS_MAX}`; `Construct::Step`; `ws/loops.rs`; `ws/steps.rs` |
| G25 | Parameter step, list | xy lt (`.step name list v…`; lt `.step temp list`), hs (`.temp t1 t2 …` only), sp (`values=[…]`), va (`values=[…]`) | Name + values in order. | `BenchCard::StepList{param,values}`; `Construct::StepList`; `Alt::TempList` |
| G26 | Circuit states between analyses | ng (`alterparam` + `reset`), va (`alter` in control), sp (`alter`, not read), hs (`.alter` blocks), xy (`.DATA` + `.STEP DATA=`), lt (index step + `table()`) | Run plan: analyses in run order, each tagged with an epoch; per-epoch delta of top-level parameter sets. | `rd/epochs.rs::Plan`; `BenchCard::Alter{key,value,node}`; `Construct::Epochs`; `Alt::{ControlScript,AlterBlocks,StatesData,StatesTable}`; W0130 |
| G27 | Analysis statement names | sp va | Spectre and VACASK analyses carry a name (`tran1 tran …`, `analysis op1 op`); SPICE cards have none. | `ast::Header::name`; `Ana::MonteCarlo::name` |
| G28 | Analysis nodes in the head | sp | `noise1 (out 0) noise`, `pz (o1 o2) pz`. | `ast::Tag::Analysis` header nodes |
| G29 | Nested / wrapped analyses | sp | `sweep … { analysis }`, `montecarlo … { analysis }`; one inner statement read, deeper nesting stays text. | `rd/named_bench.rs::{spectre_sweep,montecarlo}` |
| G30 | Several analyses per deck | ng es hs lt sp va | Xyce runs one sweeping analysis a deck (W0321). | `diag` W0321 |
| G31 | Analysis argument with no target word | — | W0040; `log` sweep the target lacks W0031. | `diag` |
| G32 | Backend analysis kinds | — | `Op Dc Ac Tran Noise Tf Sens SensAc Pz Disto Pss Hb Sp Stb TranNoise Fourier SpectreSweep SpectreMonteCarlo Pac Pnoise Pxf Pstb` (22) with routes and plot names. | `src/backend/kind.rs::Kind` |
| G33 | ngspice `.pss` parameter shape | ng | `.pss gfreq tstab oscnob psspoints harms sciter steadycoeff [uic]`: shooting for an autonomous oscillator; maps to Spectre `pss fund= tstab= harms=` + oscillator nodes, VACASK `pss tper= oscillator=1 tstab= maxharm=`. | NG §2 row 15 |
| G34 | `.sp` noise flag | ng | `.sp lin/dec n f1 f2 [donoise]`. | NG §2 row 16 |
| G35 | `.disto` | ng | Small-signal distortion; HSPICE `.DISTO Rload` computes another quantity; no other target. | NG §3 (11.3.3) |
| G36 | `.temp` list / expression on ngspice | ng hs | `.TEMP 0 25 50 75 100`, `.temp {envtemp}`; Xyce has no `.TEMP` (`.STEP TEMP LIST`). | NG §2 row 21 |
| G37 | S-parameters as a 2-port network card | lt (`.net I(Rout) Vin Rin= Rout=` + `.ac`), hs (`.NET V(out) Vin RIN= ROUT=`) | Two-port only. | NG §2 row 6 |
| G38 | `.dc` over a resistor / temperature / parameter | ng | `.dc` source may be a V/I source, a resistor, `temp` or a parameter, nested. | NG §3 (11.3.1–2) |
| G39 | LTspice `.tran` modifiers | lt | `startup` (linear 20 µs ramp of top-level DC-only V/I; waveform and subckt sources not ramped), `steady`, `nodiscard`, `step`, `uic`. | LH L1, L22 |
| G40 | LTspice `.step` targets | lt | `.step temp a b c`, `.step <src> list …`, `.step <type> <model>(<par>) …` (a model parameter), `.step param temp …` (is the temperature too). | LH L25 |
| G41 | Noise / AC over a frequency list | lt (`.noise V(a,b) src list f`, `.ac list`), hs (`.ac poi n f1 … fn`), sp va (`values=[…]`) | A list sweep kind for frequency (today `Sweep.variation` is dec/oct/lin/log only). | LH L26, H19 |
| G42 | Timed operating points | hs (`.op t1 t2 …`), sp (`tran … infotimes=[…]`) | An operating-point report at given transient times. | LH H17 |
| G43 | HSPICE `.lin` | hs | `.lin [noisecalc=1] [gdcalc=1]` over the `.ac` sweep with P ports. | LH H12 |
| G44 | HSPICE HB family | hs | `.hb f n`, `.hbac`, `.hbnoise`, `.hbxf`, `.hbosc`, `.hblin`. | LH H14; `RAWKIND` |
| G45 | HSPICE sweep clause on an analysis | hs | `.TRAN/.DC/.AC … SWEEP <param> <type> …` incl. `POI` lists, `DATA=name`, `MONTE=`; TEMP sweepable. | LH H43, §3 |
| G46 | HSPICE `.alter` | hs | Re-run with changed cards. | LH H43 |
| G47 | Sweep values from a file | lt | `.ac … file=`, `.dc … file=`, `.step … file=`. | LH §5 |
| G48 | `.four` harmonic / period counts | lt | `.four f [Nh] [Np] traces`; ngspice has `nfreqs` but no period count. | LH §5 |
| G49 | LTspice `.dc` nesting | lt | Up to three nested sources; `dec/oct/list`. | LH §5 |
| G50 | LTspice `.tran` extras | lt | `convreport` flag; `.tran` with `savestate`/`loadstate`. | LH §5 |
| G51 | Multi-tone harmonic balance | xy | `.HB f1 [f2 …]` with `.options HBINT numfreq= intmodmax= selectharms= tahb=`. | XY A1 |
| G52 | Xyce sampling / UQ | xy | `.SAMPLING param= type=normal/uniform …`, `.EMBEDDEDSAMPLING`, `.PCE`, `.options SAMPLES/PCES/EMBEDDEDSAMPLES`, `useExpr=true`. | XY A2, A3 |
| G53 | Data-table sweeps on every analysis | xy | `.STEP data=`, `.DC data=`, `.AC data=`, `.NOISE … data=` over a `.DATA` table. | XY A4 |
| G54 | Scheduled max step | xy | `.TRAN 0 T {schedule(t0,dt0,t1,dt1,…)}`: a max time step that varies with time. | XY A6 |
| G55 | `.TR` | xy | Read as `.TRAN` (not in the guide). | XY A7 |
| G56 | `.STEP` over a device parameter | xy | `.STEP dev:param …`, `.STEP TEMP …`, lin/dec/oct/list. | XY A14 |
| G57 | Xyce optimisation cards | xy | `.ROL`, `.OBJECTIVE`, `.RESULT` (ROL/Dakota). | XY A12 |
| G58 | Continuous measure modes | xy | `.MEASURE TRAN_CONT/DC_CONT/AC_CONT/NOISE_CONT`, `.MEASURE FFT`. | XY §2a; `rd/rejects.rs::MODES` |
| G59 | VACASK two-tone HB | va | `hb freq=[f1,f2] nharm=[h1,h2] truncate="box" [immax]`; `store=` / `nodeset="slot"` (named solution slots). | VS V19, V20 |
| G60 | VACASK `dcinc` details | va | `analysis dcinc` with `writeop`, `nodeset=[…]`. | VS V23 |
| G61 | VACASK `acsp`, `acstb` | va | `acsp ports=[…]`; `acstb probe=`. | VS §2 Analyses |
| G62 | VACASK sweep targets | va | A model parameter (`model="d" parameter="is"`), an instance parameter (`instance="r1" parameter="r"`), an option (`option="tran_rmax"`), a variable (`variable="v"`), the top (`instance="__topinst__"`); `continuation=1`; lin/dec/oct/step/values. | VS V9, V11–V14 |
| G63 | VACASK differential initial condition | va | `ic=["1";2.0;"2";"3";1.0]`: an entry of two nodes is the voltage across them. | VS V26 |
| G64 | VACASK `mc` block | va | `mc name samples=n … endmc`. | VS §2 Control commands |
| G65 | Spectre `dc` sweep forms | sp | `param=<netlist param>`, `lin=`, `dec=`/`log=`/`values=`, `mod=` (model parameter), `dev=r1 param=r`. | VS §2b Analyses |
| G66 | Spectre `ac`/`noise`/`tran` forms | sp | `ac param=/dev= freq=` (sweep a parameter at one frequency), `center`/`span`, `values=`; `tran` without `step`, `tran ic=…` mode; `noise oprobe=`. | VS §2b Analyses |
| G67 | Designed HB record | xy hs sp va | `Hb{tones: Vec<f64>, harms: Vec<u16>, intmod: Option<u16>, truncate: Box/Diamond}` (1–255 tones). | D-analysis-kinds §2 |
| G68 | Designed PSS record | ng sp va | `Pss{fund, tstab, harms, osc: Option<[node;2]>, points, uic}`. | D-analysis-kinds §2 |
| G69 | Designed PAC / PNoise records | sp va | `Pac{pss: run handle, sweep, sidebands}`, `Pnoise{pss, out: [node;2], src, sweep, sidebands}`: a small-signal analysis refers to the PSS run it follows (a handle, so each PSS is written once). | D-analysis-kinds §2 |
| G70 | Designed STB / ACXF records | sp va | `Stb{probe: 0 V series source name, sweep}`, `Acxf{out: [node;2], sweep}`. | D-analysis-kinds §2 |
| G71 | Analyses that normalise to existing kinds | va hs xy | `dcinc` = AC at one point of 0 Hz (`lin 1 0 0`); `dcxf` = `.tf` with its vector unwrapped; HSPICE `.lin` and Xyce `.LIN` = S-parameters. | D-analysis-kinds §2 |
| G72 | Designed run plan | — | Adds `AlterKind{Param, Instance, Model, Option, Temp}`, alter scope (subckt for `alterparam sname`), target, key, value as `Val`; loops `Values{Lin, Log{per, oct}, List}`, nesting cap 8; `base` alters folded into the netlist; normalisations N1–N5 (fold alters before the first run, drop a `type="dc"` alter pair, union `clear saves`, merge empty epochs, drop no-op alters). | D-control-epochs §2 |
| G73 | Monte Carlo inner analysis as a handle | — | `MonteCarlo{runs, seed, scope: Process/Mismatch/All, inner: run handle, name}`: today's `(name, kind)` pair loses the inner analysis's arguments. | D-mc-lowering §2 |
<!-- G-end -->

## H. Outputs

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| H1 | Save list | ng es lt (`.save …`), hs (`.probe [kind] …`; `.option post` for all), sp va (`save …`) | List of outputs or "all"; Xyce `.SAVE` is an operating-point file (inert there). Xyce/VACASK targets drop it (W0023) or write `.print <kind> V(*)`. | `ast::Tag::Save`; `BenchCard::Saves(Vec<String>)`; `Construct::Save`; `Alt::Probe` |
| H2 | Print / plot | ng es hs xy (`.print [kind] outs`, `.plot`), no LTspice | Analysis kind word (`noise disto tran sens four op dc ac tf pz` + Xyce `hb hb_fd hb_td hb_ic hb_startup homotopy es pce mpde mpde_ic mpde_startup ac_ic tranadjoint`), outputs, formats (`PRECISION=`). | `ast::Tag::Print`; `BenchCard::Print(kind,outs)`; `rd/outputs.rs::print_kind`; `ws/prints.rs` |
| H3 | Output expression forms | all | `v(n)`, `v(a,b)`, `i(src)`, ngspice `v1#branch`, `@dev[param]`, `all`; Xyce `V(*)`, `p(…)`, `w(…)`, `n(…)`, lead currents `I2(dev)`, braced expressions; HSPICE any blank-free run; Spectre `dev:p` / `inst:index` terminal currents; VACASK `v(n)`, `v(a,b)`, `i(inst)`, `p(inst,var)`, `default`, `full`. | `rd/outputs.rs`; `rd/named_bench.rs::saves`; `rd/named_control.rs::vacask_saves`; W0052 |
| H4 | Measure | ng es hs xy lt (`.meas`/`.measure`), sp (inside `simulator lang=spice`) | Kept as one line of text today (TRIG/TARG, WHEN, FIND…AT, AVG MAX MIN PP RMS INTEG, PARAM, Xyce FFT measures `ENOB EQN FIND SFDR SNDR SNR THD`). Xyce `TRIG v(1)=0.5` vs ngspice `TRIG v(1) VAL=0.5`. VACASK none (W0109); W0048. | `ast::Tag::Measure`; `BenchCard::Meas(String)`; `Construct::Measure`; `ws/xyce_cards.rs`; `src/measure_parse.rs` |
| H5 | `.control` measure/fourier commands | ng | `meas` and `fourier` as script commands read as their dot cards. | `rd/testbench.rs` (`Card::Meas`); `ws/bench.rs` |
| H6 | `.fft` | xy | Window names (19), `FORMAT=norm/unorm`, `NP`: only in the reject scanner. | `rd/rejects.rs::xyce_card` |
| H7 | Output listing cards with no numeric effect | ng (`.width`), lt (`.wave`, `.backanno`), xy (`.dcop`, `.objective`, `.probe`, stray `.ends`) | Inert: dropped with a note off their family. | `Construct::InertText`; `beh::DotRule::Note`; W0036, W0336 |
| H8 | State save/load files | lt (`.savestate .loadstate .savebias .loadbias`), hs (`.store`?) | Private text. | `d/ltspice` kept_word; `RAWKIND` |
| H9 | Result vector names | — | One canonical lowercase form across backends: nodes bare, currents `i(name)`, hierarchy `.`, `.tf` vectors named. | `src/normalize.rs` |
| H10 | HB print kinds | hs (`HB`, `HBTRAN`), xy (`HB_FD`, `HB_TD`, `HB_IC`, `HB_STARTUP`) | Phasor vs time-domain output of a harmonic balance. | `ws/prints.rs` |
| H11 | Other raw output dot words | hs… | `.graph .dout .stim .stimuli .net .biaschk .connect .mosra .pat` kept raw. | `rd/refused.rs::RAWKIND` |
| H12 | ngspice `.probe` | ng | `.probe I(R) vd(a,b) p(dev) alli`: adds 0 V sources at device nodes; device-pin forms `i(mp1:s)`, `vd(mn1:d:s)` need a terminal-name table per device; `alli` = all currents. | NG §2 row 10 |
| H13 | `par()` in outputs | ng | `.print … par('expr')`. | NG §3 (11.6.1–4) |
| H14 | Measure kinds | ng | TRIG/TARG, WHEN, FIND AT, AVG/MIN/MAX/PP/RMS, INTEG, PARAM, DERIV; a malformed measure fails only itself on ngspice. | NG §3 (11.4), §2 row 27 |
| H15 | LTspice `.net` | lt | Network parameters during `.ac` (terminations default to the source's Rser / the output resistor). | LH L27 |
| H16 | `.fourier` vs `.four` on LTspice | lt | Both print the same table; LTspice reports a zero-phase sine at +90°: a phase convention to name. | LH L29 |
| H17 | HSPICE `.fft` | hs xy | `.fft v() start= stop= np= window= alfa= format=`; ngspice has only the script `fft`. | LH H15 |
| H18 | HSPICE `.power` | hs | AVG/RMS/MAX/MIN of v·i: expressible as `.meas` on ng/xy/lt. | LH H26 |
| H19 | HSPICE checks | hs | `.check`, `.dout`, `.biaschk`: violation counts, no equivalent. | LH H27 |
| H20 | HSPICE `.load file=` | hs | Inline the `.ic`/`.nodeset` cards a `.save` wrote (a file read at import). | LH H23 |
| H21 | Measure parameters that are other measures | hs | `.meas … td=trig, from=ta`. | LH H41 |
| H22 | Xyce print formats | xy | `FORMAT=STD/NOINDEX/PROBE/TECPLOT/RAW/CSV/GNUPLOT/SPLOT`, `FILE=`, `WIDTH=`, `PRECISION=`, `FILTER=`. | XY §2f |
| H23 | Xyce device outputs | xy | Lead currents `I(x)`, `IB/IC/IE(Q)`, power `P(x)`/`W(x)`, internal variable `N(dev:var)`, a Y device's parameter `{YTRANSLINE!ERIC:len}`, `N(YRLC2!RLC1:…)`. | XY §2f, G8 |
| H24 | Operating-point files | xy | `.DCOP input=f / output=f`, `.SAVE TYPE=IC/NODESET FILE= LEVEL=`, `.INITCOND`. | XY A8 |
| H25 | VACASK small-signal port outputs | va | `save default yin(v1)`; `yin`/`zin`/`yout`/`zout`; `opdefault`, `opfull` (acstb). | VS V28, §2 Save |
| H26 | Spectre save selectors | sp | `save r1:1`, `r1:oppoint`, `r1:all`; `export` (internal use). | VS §2b Control |
| H27 | Designed output row | — | `Out{kind: V/Vdiff/I/ITerm/Power/AllCurrents/AllNodes/Expr, a, b, dev, term index}`. Terminal current spellings: ngspice `.probe i(m1:s)` or `@m1[is]`, Xyce `IS(M1)`, HSPICE `I3(M1)` (index = node order), LTspice `Is(M1)`, Spectre `M1:s`; power: `p(dev)`, `P(dev)`, Spectre `dev:pwr`; all currents: ngspice `alli`, Xyce `I(*)`, Spectre `:currents`. An unspellable output is dropped with a note, never refused. | D-probe-terminals §2, §3 |
| H28 | Designed measure records | — | `MeasureSpec{At, Find(when crossing), TrigTarg, Reduce(Avg/Min/Max/Pp/Rms/Integ over from..to), Param(expr), Deriv}`; `Crossing{vec, val, dir Rise/Fall/Cross, nth (or LAST), td}`; ngspice semantics (linear interpolation) as the reference; LTspice defaults differ. | D-vacask-postprocess §3 |
| H29 | Fourier evaluation parameters | — | Fundamental, outputs, harmonics (9), grid size (`fourgridsize`), last period, THD. | D-vacask-postprocess §3 |
<!-- H-end -->

## I. Options & simulator state

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| I1 | Options card | ng es hs lt (`.options/.option/.opt/.opts k=v flag`), xy (`.options <PKG> k=v`, `k v`, comma lists), sp (`name options k=v`), va (`options k=v` in `control`) | Ordered key/value or flag list; options merged into one card on write; Spectre's carries a statement name. | `ast::Tag::Options`; `rd/options.rs::Opt`; `beh::OptionsForm{Spice,XycePackages,Spectre,Vacask}` |
| I2 | Portable option keys | ng (the set) | `sparse klu acct noacct noinit list nomod nopage node norefvalue opts seed seedinfo temp tnom warn maxwarns savecurrents abstol gmin gminsteps itl1 itl2 keepopinfo noopiter pivrel pivtol reltol rshunt vntol rseries cshunt noopac autostop chgtol convstep convabsstep interp itl3 itl4 itl5 itl6 maxevtiter maxopalter maxord method noopalter ramptime srcsteps trtol xmu diode_cj0 diode_rser badmos3 defad defas defl defw scale trytocompact`. | `rd/options.rs::PORTABLE_OPTIONS`; `Opt::Portable` |
| I3 | Xyce package options | xy | `pkg.key` kept; mapped to portable: `DEVICE TEMP/TNOM/GMIN`, `PARSER SCALE`, `TIMEINT METHOD=gear/8`, `TIMEINT MAXORD`, `TIMEINT RELTOL`; `HBINT NUMFREQ` feeds HB. | `Opt::Xyce`; `rd/options.rs::options_view`; `ws/xyce_options.rs` |
| I4 | Spectre option names | sp | `reltol vabstol iabstol chargeabstol temp tnom dcmaxiters scale gmin pivrel pivabs` ↔ SPICE `reltol vntol abstol chgtol temp tnom itl1/max_iterations scale gmin pivrel pivtol`. | `d/spectre` option_to (12 rows); `rd/named_bench.rs::spectre_options` |
| I5 | VACASK option names | va | Same word: `reltol abstol vntol chgtol gmin temp tnom scale`; `method`→`tran_method` ("trap"/"am2", "gear2"/"bdf2"); `maxord`→`tran_maxord`; `itl1`→`op_itl`; `itl4`→`tran_itl`; `gminsteps`→`homotopy_gminsteps`; `srcsteps`→`homotopy_srcsteps`; `rshunt`→`gshunt=1/(x)`; `noopiter`→`op_skipinitial=1`; 19 cosmetic keys dropped; `rawfile` dropped. | `d/vacask` option_* rows; `beh::OptionTo{Name,Spell,Method,Cosmetic}`; `Opt::Vacask` |
| I6 | Option with no target spelling | — | Refused (W0017: Xyce VNTOL, CHGTOL, TRTOL, GSHUNT, GMINDC, ITL2, GMINSTEPS, SRCSTEPS, NOOPITER…) or noted dropped (W0024 cosmetic). | `Construct::UnmappedOption`; `ws/xyce_options.rs`; `ws/ngspice_options.rs` |
| I7 | Temperature | all | `.temp v` / `.temperature`, `.options temp=`, Xyce `.options device temp=`, Spectre `options temp=`, VACASK `options temp=`, ngspice `set temp=` / `option temp=` in a script; last one wins. | `ast::Tag::Temp`; `BenchCard::Temp`; `Opt::Temp` |
| I8 | Nominal temperature | all | `tnom`; default 27 °C (ng xy lt), 25 °C for both TNOM and TEMP on HSPICE: defaults made explicit when crossing. | `Opt::Tnom`; `beh::NetlistRules::{Ngspice,Hspice}`; `ws/ngspice_options.rs`; `ws/pair.rs` |
| I9 | Initial conditions | ng es lt (`.ic v(n)=x`), hs xy (also bare `.ic n x`; hs alias `.dcvolt`), sp (`ic n=x`), va (`ic=["n"; x; …]` on the `tran` line) | Node → value (number or expression: Xyce `.ic v(1)={V1}`). | `ast::Tag::Ic`; `BenchCard::NodeVals(true, Vec<(node,Val)>)` |
| I10 | Nodesets | ng es hs xy lt (`.nodeset v(n)=x`), sp (`nodeset n=x`), va (`nodeset=[…]` on every analysis line but `hb`) | Node → guess. | `ast::Tag::Nodeset`; `BenchCard::NodeVals(false,…)` |
| I11 | UIC | all | `uic` on `.tran`; Xyce `NOOP`; Spectre `skipdc=yes`; VACASK `icmode="uic"`. | `Ana::Tran::uic` |
| I12 | Geometry scale | ng hs xy (`scale`), sp va (`scale`) | Xyce under `PARSER`; LTspice has no SCALE option (syntax error). | `beh::NetlistRules::Ltspice`; `Opt` |
| I13 | Shunt options | ng (`rshunt`), hs (`gshunt`, `gmindc`), va (`gshunt`) | Inverse pairs; `gmindc` has no ngspice option; one GMIN is HSPICE's GMIN and GMINDC both. | `ws/ngspice_options.rs` |
| I14 | Integration method | ng (`method=trap/gear`, `maxord`), xy (`TIMEINT METHOD`), va (`tran_method`) | Method enum + order. | `beh::OptionTo::Method` |
| I15 | `ngbehavior` | ng | Compatibility mode read before the deck (`-D ngbehavior=`); written as a `*spicerack ngbehavior=` line the backend lifts; dropped elsewhere (W0030). | `Opt::NgBehavior`; `Construct::NgBehavior`; `Alt::NgBehaviorLine` |
| I16 | `savecurrents` | ng | Cosmetic on LTspice (saves all currents anyway; the option is a syntax error), ESPice, VACASK. | `d/{ltspice,espice,vacask}` option_cosmetic |
| I17 | Search path option | hs | `.option search='dir'`: used by the kit loader (≤ 40 dirs), cosmetic on write. | `res::SEARCH_MAX`; `d/vacask` option_cosmetic `search` |
| I18 | Simulator-only options | each | Xyce packages, VACASK `tran_method`…, LTspice `cshuntintern`, `plotwinsize`; only a deck in that simulator's text holds them. | `rd/options.rs::portable_option` |
| I19 | Fourier harmonic count option | ng | `.options nfreqs=`. | `src/python/deck.rs` |
| I20 | `.nodeset all=val` | ng | One value for every node; other targets need it expanded per node. | NG §3 (11.2.1) |
| I21 | LTspice option set | lt | `abstol vntol reltol chgtol trtol gmin method itl1 itl2 itl4 gminsteps srcsteps ptrantau maxstep solver plot* gshunt cshunt gfloat temp tnom numdgt measdgt list logparams logopinfo debugtran topologycheck sstol ststdelay ststclocks`. | LH §5 |
| I22 | HSPICE option families | hs | Accuracy/convergence, method/tolerance/limit/matrix, `SEARCH`, `POST`, `ACCT`, `PROBE`, `GSHUNT`, `GMINDC`, `TNOM`; GMINDC equals ngspice `gmin` only in a deck with no transient. | LH §6, H36 |
| I23 | `.keepnode` | lt | Output-only directive. | LH §5 |
| I24 | Xyce option packages | xy | `DEVICE` (TEMP, TNOM, GMIN, MINRES, MINCAP, SCALE…, SMOOTHBSRC, DIGINITSTATE), `TIMEINT` (RELTOL, ABSTOL, METHOD, MAXORD…), `NONLIN`, `NONLIN-TRAN`, `NONLIN-HB`, `LINSOL*`, `DIST`, `RESTART`, `DIAGNOSTIC`, `OUTPUT`, `LOCA` (continuation), `SAMPLES`, `EMBEDDEDSAMPLES`, `PCES`, `SENSITIVITY`, `HBINT`, `FFT`, `MEASURE`, `PARSER` (`scale=`, model binning, `hspice-ext`). | XY §2e |
| I25 | Node values in scopes | xy | `.IC`/`.NODESET` inside a `.subckt` (applied per instance), on a subckt node `V(X1:in_1)`, on a `$G_` global. | XY G6 |
<!-- I-end -->

## J. Statistics / Monte Carlo

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| J1 | `statistics { … }` block | sp | Holds `process { vary … truncate … }`, `mismatch { vary … truncate … }`, `correlate`, `truncate`; at the top or in a section (inside a subckt or a branch: refused W0018). | `ast::Tag::{Statistics,Process}`; `rd/named_blocks.rs::Stats{process,mismatch,correlations,truncate}` |
| J2 | `vary` | sp | `vary <param> dist=gauss/unif/lnorm std=<s> / N=<n> [percent=yes]`. | `rd/named_blocks.rs::Vary{param,dist,spread,percent}` |
| J3 | `correlate` | sp | `correlate param=[a b] cc=<x>` or `dev=[…]`; n>2 with expression `cc`, `dev=`, non-gauss: refused (W0018); matrix must be positive definite (W0020). | `Stats::correlations` |
| J4 | `truncate` | sp | `truncate tr=<t>` at block top, in `process`, in `mismatch`; below 4 sigma refused. | `Stats::truncate[3]` |
| J5 | Process vs mismatch scope | sp va | Process drawn once per run; mismatch per subckt instance; VACASK draws per call site per scope; `variations=` selects which. | `wn/lift.rs::{draw_scopes,left_out}` |
| J6 | SPICE draw idiom | ng hs xy | One-layer `.param p={agauss(v,s,1)}` redraws at every reference; two-layer (`p__mc` then `p=p__mc`) draws once; mismatch `p__mm` per subckt. | `r/draws.rs::{Draws,Layers}`; `cap::Rule::StatisticsDraws` |
| J7 | Lift to Spectre | sp (target) | Each statistical call `f(args)` → `rack_f(args, v)` with `parameters v=0` and a `vary v dist=gauss std=1` / `unif N=1`; five `rack_*` functions; read back by the inverse rule. | `wn/lift.rs::{Lifted,Var}`; `rd/unlift.rs::Unlift`; `cap::Rule::RackVars` |
| J8 | Lift to VACASK | va (target) | `limit` → `aunif` ternary; Xyce clamp → `min(max())`; 2-arg `agauss` gets sigma 1; a scope `montecarlo` leaves out is written at nominal. | `cap::Rule::VacaskCalls`; `rd/unlift.rs::Lift{Call,Sigma,Limit,Clamp,Nominal}` |
| J9 | Correlated draws | — | Cholesky combination of normalised two-layer draws. | `r/draws.rs` |
| J10 | Vary of an undeclared parameter | — | W0019. | `diag` |
| J11 | HSPICE `.variation` block | hs | Kept whole as lines. | `ast::Tag::Data`; `tok::KwVariation` |
| J12 | `.mc`, `.montecarlo` dot cards | ? | Raw dot words (kept text); `.mc` is the `Card` tag example. | `RAWKIND`; `ast::Tag::Card` doc |
| J13 | Seeds | sp (`seed=`), ng (`seed`, `seedinfo` options) | Integer seed on the analysis or as an option. | `Ana::MonteCarlo::seed`; `PORTABLE_OPTIONS` |
| J14 | Statistical call with no target form | — | W0003 (LTspice), W0107 (named targets). | `diag` |
| J15 | HSPICE `.variation` semantics | hs | Local/global variation blocks for Monte Carlo; map structurally to Spectre `statistics { process / mismatch }`; nominal when no MC runs. | LH H28 |
| J16 | Draw functions outside a Monte Carlo run | hs | `agauss/aunif/gauss/unif` return the nominal outside MC. | LH H37 |
| J17 | Call form vs table form | — | Call form: a parameter drawn by an expression (`.param p=agauss(1k,100,1)`; HSPICE, ngspice, LTspice `gauss`/`flat`/`mc`). Table form: a list of varied parameters apart from the netlist (Spectre `vary`, Xyce `.SAMPLING`, HSPICE `.variation`, VACASK `mc` over `parameters`). Exact rewrites both ways only when a call stands alone in its parameter and is drawn once per run. | D-mc-lowering §2 |
| J18 | Designed vary table | — | One table: parameter, `dist` (Gauss, Uniform, Lnorm; Xyce gamma refused), mean and spread as values, correlation group handle. | D-mc-lowering §2 |
<!-- J-end -->

## K. Control / scripting

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| K1 | ngspice `.control … .endc` | ng | A script: lines kept; analysis commands read as their dot cards; `pre_osdi` → load; `alterparam` + `reset` → epoch; display-only lines noted (W0025); circuit-changing unread lines refuse (W0160). In a section it is kept as one joined line. | `ast::Tag::Control`; `rd/control.rs::ControlLine{Analysis,PreOsdi,Alter,Reset,Skipped,Unread}` |
| K2 | Script display commands | ng | `asciiplot compose cutout define destroy display echo fft gnuplot hardcopy help history iplot let linearize listing plot print psd quit run rusage save setplot setscale settype show showmod spec status sysinfo undefine unlet version where wrdata write wrnodev wrs2p`. | `rd/control.rs::SHOWS` |
| K3 | Script display variables | ng | `set`/`unset` of `appendwrite askquit brief curplottitle filetype gridstyle height noaskquit nobreak nolegend nomoremode nopadding noprintscale numdgt plotstyle pointchars units width wr_singlescale wr_vecnames xbrushwidth xgridwidth`, `color<N>`, `hcopy…`. | `rd/control.rs::SHOWN_VARS` |
| K4 | Script state changes | ng | `alterparam name = v` (top form; `alterparam sub name = v` unread), `reset`, `alter`, `altermod`, `option`, `set temp=`, `let @dev[p] = …`, `source`: only the first two are read. | `rd/control.rs::control_line`; W0160 |
| K5 | Script loops and branches | ng | `foreach`, `repeat`, `while`, `dowhile`, `if`/`else`/`end` over constants are unrolled (≤ 16384 lines, W0161); variables `$v`, vectors `$&k` (six digits), `let` arithmetic; comparisons `< <= > >= = <>` and `lt le gt ge eq ne`; `break`, result-dependent loops refuse. | `rd/unroll.rs::{Flow,Step,UNROLL_MAX}` |
| K6 | Script order vs dot cards | ng | Under `-b` dot-card analyses run after the script and at each `run`; mixing with state changes is refused. | `rd/control.rs::control` |
| K7 | Control script as a target form | ng | `.step`, log/list `.dc` axes and epochs are written as one script: `foreach` loops, `alterparam`/`reset`, `alter`, `option temp=`, `dc src $d $d 1`. | `Alt::ControlScript`; `ws/{loops,epochs}.rs` |
| K8 | `.meas`/`.four` force a script | ng | A deck with `.meas` or `.four` runs its analyses from one `.control` block (batch mode has no `.measure`). | `ws/bench.rs`; `beh::ControlBlock{Ngspice,None}` |
| K9 | VACASK `control … endc` | va | Ordered statements: `options`, `sweep` (prefixes the next `analysis`), `analysis name kind k=v`, `save`, `alter`, `elaborate`, `var`, `abort`, `print`, `postprocess`. Unread ones kept as `Extra`. | `rd/named_control.rs::vacask_control`; `BenchCard::Extra`; `rd/mod.rs::Bench::Control` |
| K10 | VACASK script-only statements | va | `abort`, `print`, `postprocess`, an `embed` nothing loads: no circuit meaning (dropped W0120 elsewhere). | `Construct::VacaskScript` |
| K11 | VACASK `alter` | va | `alter instance("__topinst__") k=v …` (numbers) is read; alters of other instances or models, or expressions, stay text. | `rd/named_control.rs::vacask_alter` |
| K12 | Spectre control statements | sp | `alter`, `altergroup { … }`, `assert`, `check`, `checklimit`, `info`, `paramset`, `paramtest`, `set`, `shell`, `options`: parsed heads, kept text (alter not yet read: W0001). | `stmt::Command` (10); `ast::Tag::Command` |
| K13 | HSPICE `.alter` blocks | hs | One block per later circuit state, each setting every state parameter; not written beside an `.if` block. | `Alt::AlterBlocks`; `ws/states.rs` |
| K14 | Data-table step | xy hs (`.DATA name p1 p2 … rows … .ENDDATA`, `.STEP DATA=name`) | Table of simultaneous parameter values, one row per step. | `Alt::StatesData`; `ast::Tag::Data` |
| K15 | LTspice state table | lt | Index `.step` + each state parameter `table(index, …)`; the first step dimension is sorted, so lists lose order/repeats. | `Alt::StatesTable`; `ws/states.rs` |
| K16 | Plan invariants | — | Epoch 0 = netlist as written; an alter after a run opens the next epoch; alters after the last run are dropped; a table form holds a plan only when every state runs the same analyses. | `rd/epochs.rs::{plan,Plan::check}` |
| K17 | VACASK repeated `elaborate` | va | `elaborate circuit(...)` two or more times: epochs whose circuit (not just a parameter) differs. | VS V2 |
| K18 | VACASK `clear` | va | `clear saves`, `clear options`: resets accumulated control state between analyses. | VS V3, V4 |
| K19 | VACASK alter of an instance | va | `alter instance("v1") type="dc"` (changes a source's waveform type), `alter instance("i1") dc=-1`; `alter instance("__topinst__") a=2` may flip `@if` topology. | VS V5–V7 |
| K20 | Spectre control statements, intent | sp | `alter` (foldable when before every analysis), `altergroup`, `set` (options before the first analysis), `check`/`checklimit`/`info`/`shell` (droppable), `paramset` (a data table: Xyce `.DATA` + `.STEP DATA=`), `assert`, `paramtest`. | VS §2b Control |
<!-- K-end -->

## L. Anything else

| id | construct | native in | variants the IR must distinguish or normalize | seen at |
|---|---|---|---|---|
| L1 | Annotation directives | lt (`.backanno`) | No simulation meaning: dropped, noted (W0036). | `beh::DotRule::Note` |
| L2 | LTspice private cards | lt | `.softstart`, `.machine/.mach`, `.state`, `.rule`, `.output`, `.endmachine/.endmach`, `.savestate`, `.loadstate`, `.savebias`, `.fra`, `.ferret`, `.loadbias`, `.fourier`. | `d/ltspice` kept_word; `rd/refused.rs::dialect_raw` |
| L3 | HSPICE private cards | hs | `.stateye`, `.vec`, `.check`, `.power` (PrivateText). | `d/hspice` kept_word |
| L4 | Xyce raw cards | xy | `.embeddedsampling .temperature .preprocess .objective .sampling .initcond .result .mpde .dcop .temp .save .sens .pce .mor .rol`. | `rd/refused.rs::dialect_raw` (XY) |
| L5 | Diagnostics as data | — | Coded rows (stage letter + number), anchored to a token, `Note` vs `Refusal`; a plugin's own codes ride as a `detail`. An owned IR needs source spans to keep this. | `src/syntax/diag/mod.rs::{Code,Diag,Kind,Info}` |
| L6 | Source positions | — | Every view today is token-backed (node ids, token ids); the reader "holds node ids and the few names it makes up". | `rd/mod.rs` header |
| L7 | Kit / PDK binding facts | — | Per (file, node): scope, master binding (primitive / definition / unknown), corrected node-vs-model split, include target. Definitions: Subckt, Model (+bin), Param, Func, Section, VerilogA. | `res::{Def,DefKind,Binding,Split,Target,Scope}` |
| L8 | Python store format | py | A deck's or subckt's ngspice source text behind a first line naming its dialect ("the store is source files"). | `src/python/store.rs` |
| L9 | Python extra lines | py | `extra_line(line, dialect=…)`: a line given as one dialect's text sits in that dialect's region. | `src/python/deck.rs` |
| L10 | Simulator registry | — | Descriptors of installed simulators and the analyses/features each declares (R0040–R0042, W0180–W0182). | `src/backend/registry.rs`; `diag` |
| L11 | Pair rules (source × target) | — | Meanings fixed by the source simulator that the target would read otherwise (Xyce model `R` multiplier, `log` base, HSPICE 25 °C, first/last param, IC in DCOP…): must become explicit IR facts at read time. | `ws/pair.rs::Pair`; `beh::{ModelSemantics,NetlistRules,ExprEval}` |
| L12 | Lines its own simulator rejects | xy ng | Dropped at read with a note (P0010, P0011): the IR needs "invalid in source" rather than silence. | `rd/rejects.rs` |
| L13 | Lint | — | Netlist-level lint over rows (common mistakes, backend card warnings). | `src/lint.rs` |
| L14 | Measure/Fourier result parsing | — | Per-simulator text formats of `.meas` and `.four` results. | `src/measure_parse.rs` |
| L15 | Source text its own simulator rejects | lt | `.model … SW(… Vt=3Vh=-.5)`: LTspice itself stops; a "source fails" class distinct from foreign or unparsed text. | LH L31 |
| L16 | LTspice `.softstart` | lt | Not in the help; read only by ADI's encrypted part models. | LH L30 |
| L17 | LTspice `.wave` | lt | `.wave <file> nbits rate V(…)`: writes a .wav; output only. | LH L28 |
| L18 | Unselected in-deck library text | xy | An unselected `.lib exclude … .endl` holds lines Xyce never parses: text that must not be validated. | XY G10 |
| L19 | Template decks | xy | Dakota templates (`D1 3 0 @selecttext( dakota_DMOD, 1, DMOD1, …)`): another tool's input, not a deck. | XY §1f |
| L20 | Decks of another dialect in a corpus | xy | ngspice decks kept in the Xyce regression as oracles: dialect is a property of the file, not the folder. | XY §1g |
| L21 | VACASK option families | va | Portable: `reltol abstol vntol chgtol gmin scale temp tnom tran_method`; own: `nr_* op_* homotopy_* hb_* smsig_* solver omp relref tran_*`. | VS §2 Options |
| L22 | Invalid source text silently truncated | sp | `parameters p=1 <1 2>` (invalid Spectre) is truncated with no code today: the IR needs an error node. | VS §2b Control |
| L23 | gnucap decks in a VACASK tree | va | `benchmark/*/gnucap/runme.sim`: another dialect's files beside VACASK's. | VS V35 |
| L24 | Comments as statements (designed) | — | Today every comment is dropped (they sit in token gaps; only the title comment is written). Design: a `Comment` statement placed by position in its block, kept only on request (`Comments::{Drop, Keep}`), leader respelled per target (`*` ↔ `//`), a comment inside a continued card attaches to the next statement, a trailing comment becomes a statement after its line; schematic hints such as `*@ pos 120 40 rot 90` ride this way. | D-pdk-binding §6 |
| L25 | PDK binding file | — | `spicerack.pdk/1`: name, root (env + path), default corner; per dialect `lib`, `corners`, `includes`, `options`, `modules` (OSDI rows); device rows `{kind, flavour, cell, letter M or X, pins, defaults (e.g. b = s), params (neutral expressions over generic params: renames, unit scaling, value↔geometry), extra, needs, provenance, proof}`; cells resolve against the real PDK files in the corner's section. | D-pdk-binding §3 |
| L26 | Kit-level module table | — | `ModuleSource{binary, source, defines}` rows given beside the kit: modules loaded before any file (`.spiceinit` `osdi`, `-plugin`); Verilog-A `ifdef`s evaluated against `defines`; module names matched case-insensitively. | D-osdi-kit §2 |
| L27 | Data files as kit files | — | `Role::Data`: a file a statement reads values from (PWL files, sweep lists, `.vec`, `.load`); `DataTable{cols, values}`; the text format differs per dialect (CSV vs two columns, comment leaders, suffixes). | D-import-files §2 |
| L28 | Behaviours per statement | — | In a file with regions the reading rules (card rules, source rules, number reading, model semantics) belong to each statement's dialect, not to the file: an IR lowered per statement removes the distinction. | D-regions-ast §2 |
| L29 | Raw analysis dot words with no reader rule | hs sp (as SPICE cards) | `.acphasenoise .phasenoise .tran_noise .envelope .envlp .jitter .hbosc .hblin .hbxf .matex .snosc .qpss .pac .pss .pxf .pnoise .stb .acmatch .dcmatch .dcsens .dcinc .acxf .dcxf .montecarlo .mc .appendmodel .verilog .osdi_include .mosra`: named only in the raw-word list (kept text). | `rd/refused.rs::RAWKIND`; ZL (`.pac`, `.pnoise`, `.snosc`) |
| L30 | Constructs planned in the ledger, absent from batch-22 | — | `Construct::{Mesfet, OptionScale, PulseCycles, ScopedSweep, SinCycles, SourceInvalid}`; views `Card::{Rlgc, Ltra, Pss, Power, NPort, Net, Laplace, Gate, Flux, Fft, Delay, Data}`, `BenchCard::Fft`, `Opt::TranRmax`, `Ana::{Pss, Pac, Acxf}`; `Alt::{PwlCycles, BSource}`. | ZL (P3-011, P5-010, P6-001, P8-038 rows) |
| L31 | Codes planned in the ledger, absent from the catalogue | — | W0044, W0131–W0136, W0138, W0139, W0183–W0189 (PDK binding: W0183 a cell the PDK lacks, W0186 pin count), R0043, R0044, P0050 (`gen__` prefix). | ZL; D-pdk-binding §3, §5 |
| L32 | Scoped sweep in the API | py sp va | A sweep around named analyses (`add_sweep(name, sweep, analyses=[h])`), replacing the Spectre-only form: analyses need handles. | ZL P8-038 |
| L33 | VACASK tolerance options | va | `tran_lteratio` (= `trtol`), `tran_rmax` (max step ratio), `tolscale` (scaled tolerances). | ZL (`Opt::TranRmax` row) |
<!-- L-end -->

## M. Capability matrix (what each target does with each `Construct`)

N native, S(x) spelled as alternative x, L(x) lifted by rule x, R(code) refused, O(code) omitted with a note, C(code) copied with a note. es = ngspice's cell unless shown. Source: `d/*.rs` capability columns.

| Construct | ng | es | hs | xy | lt | sp | va | cdl | vlog |
|---|---|---|---|---|---|---|---|---|---|
| ForeignText | R W0001 | = | R W0001 | R W0001 | R W0001 | R W0001 | R W0001 | O W0028 | R W0001 |
| PrivateText | R W0039 | = | R W0039 | R W0039 | R W0039 | R W0039 | R W0039 | C W0027 | R W0039 |
| XyceSens | R W0015 | = | R W0015 | N | R W0015 | R W0015 | R W0015 | C W0027 | R W0001 |
| HspiceAnalysis | R W0015 | = | N | R W0015 | R W0015 | R W0015 | R W0015 | C W0027 | R W0001 |
| VacaskScript | O W0120 | = | O W0120 | O W0120 | O W0120 | O W0120 | N | C W0027 | O W0120 |
| CaretReread | R W0002 | = | R W0002 | R W0002 | R W0002 | R W0002 | R W0002 | R W0002 | R W0002 |
| StatisticalCall | N | = | N | N | R W0003 | L RackVars | L VacaskCalls | N | N |
| VerilogALoad | S PreOsdi | S Hdl | S Hdl | R W0004 | R W0004 | N | N | N | N |
| OsdiLoad | S PreOsdi | R W0004 | R W0004 | R W0004 | R W0004 | R W0004 | N | N | N |
| OsdiInstance | N | R W0004 | R W0004 | R W0004 | R W0004 | N | N | N | O W0029 |
| LibWholeFile | S Include | = | S Include | S Include | N | N | N | R W0005 | R W0005 |
| NgBehavior | S NgBehaviorLine | O W0030 | O W0030 | O W0030 | O W0030 | O W0030 | O W0030 | O W0030 | O W0030 |
| LibSection | N | = | N | N | R W0012 | N | N | N | N |
| RclModel | N | = | N | N | R W0006 | R W0006 | R W0006 | N | N |
| ChargeCapacitor | S ChargeQuoted | = | S ChargeQuoted | S ChargeBraced | S ChargeBare | S BsourceCharge | N | N | N |
| BehaviouralSource | N | = | S VolCur | S BracedBehaviour | N | S Bsource | N | N | N |
| Laplace | L LaplaceXfer | L LaplaceTable | L LaplaceTable | R W0011 | L LaplaceAttr | R W0011 | R W0011 | L LaplaceXfer | R W0011 |
| RfPort | N | = | R W0007 | S XycePort | R W0007 | N | N | N | N |
| PwlRepeatV | N | = | R W0008 | R W0008 | R W0008 | S PwlPeriod | R W0008 | N | R W0008 |
| PwlRepeatI | R W0008 | N | R W0008 | R W0008 | R W0008 | S PwlPeriod | R W0008 | R W0008 | R W0008 |
| NoiseWave | N | R W0009 | R W0009 | R W0009 | R W0009 | R W0009 | R W0009 | N | R W0009 |
| Conditional | N | = | N | R W0013 | R W0013 | N | N | N | N |
| BranchParam | R W0014 | = | R W0014 | R W0014 | R W0014 | N | R W0014 | N | N |
| Func | N | = | S FuncParam | N | N | N | R W0106 | N | N |
| LocalFunc | N | = | N | N | R W0106 | R W0106 | R W0106 | N | N |
| TableSource | S PwlValue | S TernaryValue | R W0102 | N | R W0102 | R W0102 | R W0102 | N | N |
| LossyLine | N | = | R W0105 | N | N | R W0105 | R W0105 | N | N |
| AmWave | N | S AmNoOffset | S HspiceAm | R W0108 | R W0108 | R W0108 | N | N | N |
| Step | S ControlScript | R W0016 | R W0016 | N | N | N | N | N | N |
| StepList | S ControlScript | R W0016 | S TempList | N | N | N | N | N | N |
| TranStartup | L StartupRamp | = | L StartupRamp | L StartupRamp | N | L StartupRamp | L StartupRamp | N | N |
| DcLogList | S ControlScript | R W0038 | S DcType | N | N | N | N | N | N |
| Tf | N | = | N | R W0015 | N | N | N | N | N |
| Sens | N | = | N | R W0015 | R W0015 | N | R W0015 | N | N |
| SensAc | N | = | R W0015 | R W0015 | R W0015 | N | R W0015 | N | N |
| PoleZero | N | = | S HspicePz | R W0015 | R W0015 | N | R W0015 | N | N |
| SParameter | N | = | R W0015 | S XyceLin | R W0015 | N | N | N | N |
| Print | N | = | N | N | O W0023 | N | N | N | N |
| Save | N | = | S Probe | O W0023 | N | N | O W0023 | N | N |
| UnmappedOption | N | = | N | R W0017 | N | N | R W0017 | N | N |
| Statistics | L StatisticsDraws | = | L StatisticsDraws | L StatisticsDraws | L StatisticsDraws | N | L StatisticsDraws | N | N |
| Measure | N | = | N | N | N | S SpiceRegion | R W0109 | N | N |
| HspiceWLine | R W0105 | = | N | R W0105 | R W0105 | R W0001 | R W0105 | C W0027 | R W0001 |
| VacaskElaborate | R W0122 | = | R W0122 | R W0122 | R W0122 | R W0122 | N | C W0027 | R W0122 |
| Epochs | S ControlScript | R W0130 | S AlterBlocks | S StatesData | S StatesTable | N | N | O W0028 | O W0029 |
| InertText | O W0336 | = | O W0336 | O W0336 | O W0336 | O W0336 | O W0336 | O W0028 | O W0029 |
| HarmonicBalance | R W0015 | N | N | N | R W0015 | N | N | N | N |
<!-- M-end -->

---

## View types today (`src/syntax/render/read/`)

All are `pub(in crate::syntax::render)` unless marked (p) private / `pub(super)`. Statement handles are AST node ids (`u32`); nearly every string is spelled from tokens on demand.

| type | file | variants / fields |
|---|---|---|
| `Rd<'a>` | `mod.rs` | reader state: `ast`, `t` (tokens), `d: Lang`, `b: &Behaviours`, `models` (name → card), `kinds` (named model kinds), `subckts`, `insts` ((scope,name) → node), `scope_of`, `kit`, `masters` (name map of X masters), `unlifted`, `fixes` (token → `Fix`), `target` spelling, `nums`, `global_params`, `promoted`, `merge_units`, `ramped`, `removed` |
| `Deck` | `mod.rs` | `top: Block`, `defs: Vec<Block>` (hoisted subckts), `sections: Vec<Section>`, `globals: Vec<String>`, `funcs: Vec<Func>`, `bench: Vec<Bench>`, `notes: Vec<Diag>`, `given_loads: Vec<String>`, `numbers: Numbers` |
| `Bench` | `mod.rs` | `Card(n)`, `Set(n, f64)`, `Meas(n)`, `Save(n)`, `Analysis(n)`, `Options(n)`, `NodeVals(n)`, `Control(n)` |
| `Block` | `mod.rs` | `name`, `node`, `comps: Vec<Comp>`, `insts: Vec<(place, n)>`, `conds: Vec<(place, Cond)>`, `models: Vec<(n, bin)>`, `params`, `locals`, `funcs: Vec<Func>`, `includes`, `libs`, `raw: Vec<Raw>`, `loads`, `own_loads`, `statistics` |
| `Comp` / `Elem` | `mod.rs` | `Comp::{Card(n), Raw(n)}`; `Elem::{Comp(Comp), Inst(n), Cond(idx)}` |
| `Raw` / `Kept` | `mod.rs` | `Raw::{Line(n), Lines(n), Joined(n)}`; `Kept::{Raw, Unparsed}` |
| `Cond` | `mod.rs` | `branches: Vec<(head node, Block)>` |
| `Section` | `mod.rs` | `node`, `library: Option<String>`, `body: Block`, `defs: Vec<Block>`, `funcs: Vec<Func>` |
| `Func` | `mod.rs` | `Param(n)` (`.param f(x)=`), `Card(n)` (`.func`), `Real(n)` (Spectre `real`) |
| `Scope` (p) / `Acc` (p) | `mod.rs` | `Scope::{Top, Subckt, Section}`; `Acc{globals, funcs, control_loads, defs, sections, bench, notes}` |
| `Card` | `elements.rs` | `Rcl{letter,name,n1,n2,value: Val,model: Option<String>,params}`, `Charge{name,n1,n2,charge,value: node,params}`, `Table{current,name,np,nm,expr,points: Vec<(f64,f64)>}`, `Coupling{name,ls,refs,k: f64}`, `Source{letter,name,np,nm,value: Val,ac_mag: Option<f64>,ac_phase: Option<f64>,wave: Option<Wave>,port: Option<(u32,f64)>,parasitics}`, `Behavioural{current,name,np,nm,expr,value: node}`, `Linear{letter,name,nodes,gain: f64}`, `Poly{letter,name,np,nm,controls,coeffs: Vec<f64>}`, `Semi{letter,name,nodes,model,params}`, `Switch{letter,name,nodes,model}`, `Line{name,nodes,z0: f64,td: f64}`, `Inst{name,ports,master,params}` (12 variants; `params: Vec<(String,String)>`) |
| `Val` | `values.rs` | `Num(f64)`, `Expr(String)`, `Raw(String)` |
| `Num` | `sources.rs` | `Num(f64)`, `Expr(String)` |
| `Wave` / `WaveKind` | `sources.rs` | `Wave{kind, args: Vec<Num>, repeat: Option<Num>, delay: Option<Num>, cut: bool}`; `WaveKind::{Pulse, Sin, Pwl, Exp, Sffm, Am, Trnoise, Trrandom}` (held counts 8, 7, all, 6, 5, 5→8 for AM args, 7, 5) |
| `Spec` (p) | `sources.rs` | `Dc(Option<n>)`, `Ac(Vec<n>)`, `Wave(WaveKind, Vec<n>)`, `Pair(String, n)` |
| `Head` / `Model` | `cards.rs` | `Head{ports: Vec<String>, params: Vec<(String,String)>}`; `Model{name, kind, params: Vec<(String,String)>}` (+ `Model::level()`) |
| `BenchCard` | `testbench.rs` | `Ana(Ana)`, `Options(Vec<Opt>)`, `Saves(Vec<String>)`, `Step{sweep: Option<&str>, param, nums: [f64;3]}`, `StepList{param, values}`, `Temp(f64)`, `NodeVals(is_ic, Vec<(String, Val)>)`, `Print(kind, outs)`, `Meas(String)`, `Alter{key, value: f64, node}`, `Extra(n)` |
| `Ana` | `testbench.rs` | `Op`, `Dc(Vec<(src,start,stop,step)>)`, `DcAxes(Vec<DcAxis>)`, `Ac(Sweep)`, `Sp(Sweep)`, `Tran{step,stop,start,max_step,uic,startup}`, `Noise{output,reference,source,sweep,pps}`, `Tf(out,src)`, `Sens(out, Option<Sweep>)`, `Pz([String;4], tf, kind)`, `Four(f, outs)`, `Hb{tone, harms: u16}`, `MonteCarlo{name,runs,seed,variations,inner}`, `Spectre{kind, node}` (14 variants) |
| `Sweep` | `testbench.rs` | `variation: &'static str` (dec/oct/lin/log), `points: u32`, `start`, `stop` |
| `Card` (p, shape of a testbench card) | `testbench.rs` | `Op`, `Dc`, `DcAxes`, `Freq`, `Tran`, `Noise`, `Tf`, `Sens`, `Pz`, `PzNodes`, `Four`, `Hb`, `Options`, `Temp`, `NodeVals`, `Save`, `Probe`, `Print`, `Meas`, `Step`, `StepList` (21) |
| `DcAxis` / `DcSweep` | `sweeps.rs` | `DcAxis{src, sweep}`; `DcSweep::{Lin(start,stop,step), Log(Sweep), List(Vec<f64>)}` |
| `Opt` | `options.rs` | `Portable(k,v)`, `Xyce(pkg.key, v)`, `Temp(f64)`, `Tnom(f64)`, `MaxStep(f64)`, `Vacask(k,v)`, `NgBehavior(String)` |
| `Stats` / `Vary` | `named_blocks.rs` | `Stats{process: Vec<Vary>, mismatch: Vec<Vary>, correlations: Vec<(params, devs, cc)>, truncate: [Option<String>;3]}`; `Vary{param, dist: gauss/unif/lnorm, spread, percent}` |
| `Plan` | `epochs.rs` | SoA: `runs: Vec<Ana>`, `run_tok`, `run_epoch`, `epoch_alters`, `alter_key`, `alter_value: Vec<f64>`, `alter_tok` |
| `ControlLine` (p) | `control.rs` | `Analysis`, `PreOsdi(String)`, `Alter`, `Reset`, `Skipped`, `Unread` |
| `Step` (p), `Flow` (p) | `unroll.rs` | `Step{line, value: Option<f64>}`; `Flow::{Foreach, Repeat, While, Dowhile, If, Else, End, Plain}` |
| `Unlift` (p) | `unlift.rs` | `calls: token → (function, cut token)`, `used: HashSet<String>` |
| `Fix` / `Step` / `Lift` | `unlift.rs` | `Fix::{Respell(String), Lift(Lift), Inline(Vec<(tok, Step)>)}`; `Step::{Inline(a,b), Subst(a,b), Call(Option<Lift>)}`; `Lift::{Call{f,sigma,var}, Sigma(f), Limit, Clamp, Nominal}` |
| `Opened` (p) | `blocks.rs` | `Subckt(n)`, `If(n)`, `Dissolve(n)`, `Library(n)`, `Promoted(n)` |
| `Visit<'b>` / `Walk` | `walk.rs` | `Comp(&Block, Comp)`, `Inst(&Block, n)`, `Cond(&Block, &Cond)`, `Branch(&Cond, idx)`, `EndCond`, `EndBlock(&Block)` |
| `Keys` (p), `Declared` (p) | `named_elements.rs` | per-dialect key names `{source, gm, rm, probe, mutual, coupling}`; `Declared::{Subckt, Model(kind)}` |
| `Pwl` (p) | `rejects.rs` | `points`, `repeat`, `expr`, `file` (reject scanner only) |

Writer-side helper types the views feed (names only; fields not inventoried): `render::{Source, Numbers, Written, Diags, Reference}`, `swept::Swept`, `draws::{Draws, Layers}`, `names::Names`, `models::{Param, Row, Implied, Kind}`, `spelling::{Spelling, FuncForm}`, `spice::pair::Pair`, `spice::laplace::Laplace`, `spice::states::States`, `spice::loops::Loop`, `spice::powers::{Grouping, Law}`, `spice::ternary::{Read, Nest}`, `spice::xyce_models::Shape`, `named::lift::{Lifted, Var}`, `named::bench::{Tb, Signal}`, `named::sites::{Site, Scoped}`.

Kit side (`src/syntax/resolve/mod.rs`): `Kit`, `File{path, role, text, diags}`, `Form{Deck, Library}`, `Role{Deck, Library, VerilogA, Unread(LoadError)}`, `Def{name, kind, file, node, scope}`, `DefKind{Subckt{ports,params}, Model{word,kind,level,polarity}, Param, Func, Section, VerilogA{ports,params}}`, `Polarity{N,P}`, `Scope{Top, Subckt, Section, Branch}`, `Binding{Primitive, Def, Unknown}`, `Split{nodes, master}`, `Target{file, section}`.

## Counts

Counted by reading or by `grep`/`awk` over batch-22 (no build run).

| what | count | source |
|---|---|---|
| `Construct` variants | 47 | `capability.rs` (pinned by `construct_order_is_pinned`) |
| `Action` / `Alt` / `Rule` variants | 4 / 30 / 7 | `capability.rs` |
| `Primitive` variants | 26 | `device.rs` (`Primitive::COUNT`) |
| device table rows | SPICE 20, NGSPICE_OWN 6, LOSSY_LINE 1, XYCE_PORT 1, SPECTRE 22, VACASK 25 | `device.rs` |
| `Analysis` kinds | 40 | `statement.rs` (`Analysis::COUNT`) |
| `Command` kinds (Spectre control statements) | 10 | `statement.rs` |
| backend `Kind` (runnable analysis kinds) | 22 | `src/backend/kind.rs` |
| AST `Tag` variants | 51 (33 structure/statement, 5 item, 13 expression incl. `Bad`) | `ast.rs` |
| token `Tag` variants | 108 | `token.rs` |
| diagnostic codes L | 0 | `diag/rows_lp.rs` (range reserved) |
| diagnostic codes P | 18 (all notes) | `rows_lp.rs` |
| diagnostic codes R | 13 (all notes) | `rows_r.rs` |
| diagnostic codes W | 84 (61 refusals, 23 notes): W0001–W0100 49, W0101–W0139 23, S1 3, S2 2, S3 3, def 2, S4 2 | `rows_w_*.rs`, `rows_s*.rs` |
| behaviour enums | 22 `pub enum` in `behaviour.rs`: 19 selectors (`Writer NumberReading ModelSemantics SourceRules CardRules NamedRules OptionsForm LoadForm ControlBlock AnalysisForm EndRule ValueQuoting Coupling IncludeScope ParamForm ExprEval NetlistRules Parasitics NamedParams`) + `OptionTo`, `DotRule`, `MasterAnswer` | `behaviour.rs` |
| `Behaviours` fields | 36 | `behaviour.rs` |
| lexicon enums / `Lexicon` fields | 9 (`Join Dollar Backslash Quote Caret Nesting Scale Tail Arithmetic`) / 38 | `lexicon/mod.rs` |
| shared SPICE keywords | 47 spellings | `lexicon::SPICE_KEYWORDS` |
| raw dot words | 92 (+ LTspice 15, Xyce 15 overlay words) | `read/refused.rs` |
| portable option keys | 60 | `read/options.rs::PORTABLE_OPTIONS` |

Dialect definition calls (builder calls in `src/syntax/dialects/<name>.rs`; "source calls" = call sites counted by grep, "rows" = after expanding the `for` loops over const tables; every non-extending dialect answers all 47 capability cells):

| dialect | source calls | rows (approx.) | capability cells | notable row groups |
|---|---|---|---|---|
| ngspice | 189 (+2 on the `new` line) | ≈ 208 | 47 | builtin_device 57, comment_line 18, infix 17, scale 11, keyword 3, kept_word 2 |
| espice (extends ngspice) | 25 | 25 | 14 overrides | behaviours 8, option_cosmetic 1 |
| hspice | 133 | ≈ 150 | 47 | kept_word 14, infix 14, scale 12, operator 7, spectre_analysis 6, vacask_analysis 3, level_alias 2, dot_rule 1 |
| xyce | 196 | ≈ 208 | 47 | kept_word 53, infix 18, probe 15, scale 11, keyword 3, dot_rule 3, level_alias 2 |
| ltspice | 132 | ≈ 148 | 47 | kept_word 16, infix 15, scale 11, operator 5, keyword 3, dot_rule 1 |
| spectre | 165 | ≈ 248 | 47 | master 45 (34 analyses, 10 commands, options), keyword 26, spice_model_type 19, scale 15, infix 14, model_master 13, option_to 12, vacask_analysis 5, region 1 |
| vacask | 248 | ≈ 289 | 47 | model_master 30, builtin_constant 22, keyword 21, spice_model_type 20, option_cosmetic 19, infix 18, scale 15, option_to 13, spectre_analysis 11, vacask_analysis 10, analysis 8, model_load 3, option_spell 2 |
| cdl (writer only) | 56 | 56 | 47 | — |
| verilog (writer only) | 56 | 56 | 47 | — |
| ngspice-given (extends ngspice) | 11 | 11 | 0 | region 6 |
| spicerack (extends ngspice-given) | 32 | 32 | 0 | infix 15, nonassoc 6 |
