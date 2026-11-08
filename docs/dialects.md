# Dialect plugins

A dialect is a definition: the calls that say how its text reads (comment
characters, keywords, number suffixes), how decks are written in it, what
it does with each construct another dialect may hold, and, for what no row
says, a hook that writes one kind of card. Every built-in (ngspice, HSPICE,
Spectre, ...) is declared with these calls in Rust; a plugin makes the same
calls from Python, usually extending a built-in and changing a few rows.
A registered plugin is a name `Testbench.netlist`, `Testbench.from_source`
and `extra_line(..., dialect=)` accept.

## A small dialect

Acme is ngspice with `#` comment lines and `;` tail comments, `.subcircuit`
and `.endsub` for `.subckt` and `.ends`, `x` for mega, functions written as
`.param f(x)='body'`, no Laplace sources (its own refusal code), charge
capacitors as `Q={expr}`, `reltol` spelled `rtol`, and a `.noise` card of
its own form, written by a hook:

```python
import spicerack as ps
from spicerack.def_ import (Lang, Tag, Scale, Arithmetic, NumberReading, FuncForm,
                            Construct, Alt, Kind, Answer)

class Acme(ps.Dialect):
    def define(self, d):
        return (d.extends(Lang.NGSPICE)
                 .comment_line("#")
                 .comment_tail(";")
                 .keyword(".subcircuit", Tag.KwSubckt)
                 .keyword(".endsub", Tag.KwEnds)
                 .write_head(Tag.KwSubckt, ".subcircuit")
                 .write_head(Tag.KwEnds, ".endsub")
                 .scale("x", Scale.Exp(6))
                 .arithmetic(Arithmetic.Decimal)
                 .number_reading(NumberReading.Decimal)
                 .ngspice_numbers(False)
                 .func(FuncForm.Param)
                 .spell(Construct.Func, Alt.FuncParam)
                 .option("reltol", "rtol")
                 .code("ACME001", Kind.Refusal, "acme has no Laplace sources")
                 .code("ACME002", Kind.Refusal, "acme's .noise takes one source")
                 .refuse_code(Construct.Laplace, "ACME001")
                 .spell(Construct.ChargeCapacitor, Alt.ChargeBraced)
                 .hook(".noise", self.write_noise))

    def write_noise(self, cards):          # ps.Card: head, words, params, text
        return [Answer.Text(f".noise_acme out={c.words[0]} src={c.words[1]} "
                            + " ".join(c.words[2:]))
                if len(c.words) >= 2 else Answer.Refuse("ACME002")
                for c in cards]

acme = ps.register_dialect(Acme("acme-doc"))
```

`register_dialect` builds `DialectDef(name)`, applies the class's
one-argument attributes (`extends = Lang.NGSPICE`, `fold_case = True`:
any call of one argument), calls `define`, and registers the result. It
returns the dialect's `Lang`; registering an equal definition again
returns the same one, and another definition under a registered name
raises `ValueError`.

Using it is naming it:

```python
deck = """* amp
.subckt load n PARAMS: r=1k
R1 n 0 {r}
.ends load
V1 in 0 1 AC 1
R1 in out 1k
X1 out load r=2k
.options reltol=1e-4
.noise v(out) V1 dec 10 1 1meg
.end
"""
tb = ps.Testbench.from_source(deck, dialect="ngspice")
text = tb.netlist("acme-doc")                       # ngspice -> acme
assert ".subcircuit load n PARAMS: r=1k" in text
assert ".noise_acme out=V(out) src=V1 dec 10 1 1meg" in text
assert ".options rtol=1e-4" in text

own = "# t\n.subcircuit load n\nR1 n 0 1x ; one meg\n.endsub load\nX1 a load\n.op\n.end\n"
print(ps.Testbench.from_source(own, dialect="acme-doc").netlist("ngspice"))   # acme -> ngspice

laplace = ps.Testbench.from_source(
    "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s))\nR1 out 0 1k\n.op\n.end\n")
try:
    laplace.netlist("acme-doc")
except RuntimeError as e:
    assert "W0201 acme-doc:ACME001 acme has no Laplace sources" in str(e)
```

A refusal is coded and positioned like a built-in's: `W0201` (a
definition's refusal) or `W0202` (its note), then `<dialect>:<id>` and the
definition's message.

## Extending a built-in

`spicerack.dialects` holds the built-ins as classes (`NGSPICE`, `ESPICE`,
`HSPICE`, `XYCE`, `LTSPICE`, `SPECTRE`, `VACASK`, `CDL`, `VERILOG`), each
`define` being `d.extends(Lang.X)`; a subclass calls `super().define(d)`
and adds its rows. A plugin extends a plugin the same way.

```python
class MyHspice(ps.dialects.HSPICE):
    def define(self, d):
        return (super().define(d)
                .code("MY001", Kind.Refusal, "our HSPICE build has no .step")
                .refuse_code(Construct.Step, "MY001"))

my = ps.register_dialect(MyHspice("my-hspice-doc"))
assert my.dump().count("MY001") == 2              # the code and its cell
```

## Running a plugin's deck

A plugin is netlist-only: `run()` with `with_backend("acme-doc")` raises
naming that no simulator runs it. `runs_on(Simulator.X)` claims that
simulator reads the dialect's text; registration allows it only when
`X`'s own dialect is in the `extends` chain. The deck is then written in
the plugin's dialect and run there:

```python
from spicerack.def_ import Simulator

class MyNgspice(ps.dialects.NGSPICE):
    def define(self, d):
        return super().define(d).runs_on(Simulator.Ngspice)

ps.register_dialect(MyNgspice("my-ngspice-doc"))
```

## The calls

`spicerack.def_.DialectDef` has one method per Rust call of
`spicerack::syntax::def::DialectDef`, same name, same arguments, each
returning the definition: identity (`extends`, `renamed`), lines (`title`,
`fold_case`, `comment_line`, `comment_col1`, `comment_tail`, `cont_skip`,
`dollar`, `plus`, `backslash`, `end`, `stop`, `join`, ...), words
(`quote`, `string`, `ident_start`, `directive`, `keywords`, `keyword`,
...), expressions (`operator`, `infix`, `prefix`, `caret`, `probe`),
numbers (`exponent`, `scale`, `tail`, `arithmetic`), elements (`devices`,
`device`, `master`, `analysis`, `region`), spelling (`comment`,
`path_quote`, `expr`, `func`, `subckt_params`, `write_head`,
`write_master`, `option`), the named writers' maps (`option_to`,
`option_method`, `option_cosmetic`: what a SPICE `.options` key is on a
Spectre or VACASK target; `model_master(kind, levels, master, type_)` and
`spice_model_type(kind)`: the master a SPICE `.model` type gets at a
LEVEL, `levels` a list), the capability cells (`native`, `spell`,
`lift`, `refuse`, `omit`, `copy`, `refuse_code`, `omit_code`), the
behaviours (`writer`, `card_rules`, `control_block`, ...), `code`, `hook`
and `runs_on`. `ps.def_.register(d)` registers a `DialectDef` directly.

The vocabulary is the Rust enums by name: `Tag.KwSubckt` (also
`Keyword`), `Construct.Laplace`, `Alt.ChargeBraced`, `Scale.Exp(6)`,
`Alt.Omit(Code.W0030)`. A variant named `None` is `None_` (`Join.None_`).
`Lang.NGSPICE` ... `Lang.VERILOG` are the built-ins; `Lang.dump()` is a
registered dialect's every table as text.

A hook takes the cards of one head as the plugin's writer wrote them
(`Card.head`, `.words`, `.params` (`name=value` pairs), `.text`) and
returns one `Answer` per card: `Answer.Text(text)` replaces the card,
`Answer.Refuse(id)` refuses the deck with the definition's code `id`. It
is called once per deck with every card of its head. A hook is held from
the registration that makes its dialect's row, for the life of the process
(a row is never freed: one hook object per hooked registration, at most
`LANGS_MAX` rows); a definition never registered, or registered again
equal, holds none. Its text is written,
never read back: a deck holding a hooked card reads back in the plugin's
dialect only if the hook writes text the definition reads.
