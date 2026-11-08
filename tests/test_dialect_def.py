"""The Python dialect-definition API (`spicerack.def_`, `spicerack.dialects`;
DIALECT-PLUGIN.md §2, §4, §5).

The registry is one per process: each test registers its plugins under
names of its own (`uniq`), so no test sees another's and their order
cannot matter.

Oracles: the Rust rows' pinned dumps (`tests/fixtures/dialects/*.dump`,
written by `tests/test_dialect_plugin.rs` from the same calls in Rust):
acme and `MyHspice` declared here register rows whose `dump()` equals the
Rust ones byte for byte, and every built-in declared again here with its
Rust file's calls (`builtin_dialects.py`) dumps its Rust row, names aside
(oracle 3); the Rust variants' names (`variants()`,
from Rust's `Debug`) against each Python class's members (oracle 9); the
catalogue's `W0201` message with the definition's id; ngspice itself for a
plugin that runs on it (the same operating point as the ngspice deck).
"""
import re
import sys
from pathlib import Path

import pytest

import spicerack as ps
from spicerack.def_ import (Alt, Answer, Arithmetic, Construct, FuncForm, Kind, Lang,
                            NumberReading, Scale, Simulator, Tag)

from builtin_dialects import BUILTINS

PINS = Path(__file__).resolve().parent / "fixtures" / "dialects"
CALLS = []


class Acme(ps.Dialect):
    """§2's acme, the calls of `tests/test_dialect_plugin.rs`'s `acme_as`."""

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

    def write_noise(self, cards):
        """Refuses a card whose source is not `V1` (acme's one source)."""
        CALLS.append(len(cards))
        return [Answer.Text(f".noise_acme out={c.words[0]} src={c.words[1]} " + " ".join(c.words[2:]))
                if len(c.words) >= 2 and c.words[1] == "V1" else Answer.Refuse("ACME002")
                for c in cards]


class MyHspice(ps.dialects.HSPICE):
    def define(self, d):
        return (super().define(d)
                .comment_tail("//")
                .code("MY001", Kind.Refusal, "our HSPICE build has no .step")
                .refuse_code(Construct.Step, "MY001"))


@pytest.fixture
def uniq(request):
    """`uniq(base)`: a dialect name only this test registers."""
    test = re.sub(r"[^A-Za-z0-9]+", "-", request.node.name).strip("-")
    return lambda base: f"{base}-{test}"


@pytest.fixture
def acme(uniq):
    """§2's acme, registered under this test's own name."""
    name = uniq("acme")
    ps.register_dialect(Acme(name))
    return name


def test_acme_dumps_the_rust_row(acme):
    assert Lang.from_name(acme).dump().replace(acme, "acme") == (PINS / "acme.dump").read_text()


def test_my_hspice_dumps_the_rust_extends(uniq):
    name = uniq("my-hspice")
    got = ps.register_dialect(MyHspice(name)).dump().replace(name, "my-hspice")
    assert got == (PINS / "my-hspice.dump").read_text()


@pytest.mark.parametrize("builtin", sorted(BUILTINS))
def test_a_built_in_declared_from_python_dumps_the_rust_row(builtin, uniq):
    calls, rust = BUILTINS[builtin]

    class Again(ps.Dialect):
        def define(self, d):
            return calls(d)

    name = uniq(rust.name())
    want = rust.dump().splitlines()
    got = ps.register_dialect(Again(name)).dump().splitlines()
    assert len(got) == len(want)
    for w, g in zip(want, got):
        if w != g:
            # Only the rows naming the language itself differ.
            assert w.split(" ")[0] in ("name", "lex.lang", "sp.lexicon", "behaviours"), (w, g)
            assert g.replace(name, rust.name()) == w


def test_registration_is_idempotent_and_refuses_another_definition(acme):
    assert ps.register_dialect(Acme(acme)) == Lang.from_name(acme)

    class Other(Acme):
        def define(self, d):
            return super().define(d).scale("y", Scale.Exp(3))

    with pytest.raises(ValueError, match="registered with another definition"):
        ps.register_dialect(Other(acme))


def test_class_attributes_are_one_argument_calls(uniq):
    class Sugar(ps.Dialect):
        extends = Lang.NGSPICE
        fold_case = False

    a, b = uniq("sugar"), uniq("sugar-calls")
    sugar = ps.register_dialect(Sugar(a))
    want = ps.def_.register(ps.def_.DialectDef(b).extends(Lang.NGSPICE).fold_case(False))
    assert sugar.dump().replace(a, "X") == want.dump().replace(b, "X")


def test_a_hook_is_held_once_per_registered_row(uniq):
    """One Python hook object per new row: none for a definition never
    registered, none for an equal re-registration (rows are never freed,
    so each registration with a hook holds its hook for good)."""
    def hook(cards):
        return [Answer.Text(c.text) for c in cards]

    def with_hook(name):
        return ps.def_.DialectDef(name).extends(Lang.NGSPICE).hook(".noise", hook)

    before = sys.getrefcount(hook)
    names = [uniq(f"hooked{i}") for i in range(3)]
    for name in names:
        ps.def_.register(with_hook(name))
        ps.def_.register(with_hook(name))
    with_hook(uniq("never-registered"))
    assert sys.getrefcount(hook) == before + len(names)


ENUMS = [n for n in dir(ps.def_) if hasattr(getattr(ps.def_, n), "variants")]


@pytest.mark.parametrize("name", ENUMS)
def test_enum_members_are_the_rust_variants(name):
    e = getattr(ps.def_, name)
    members = {m for m in vars(e) if not m.startswith("_") and m != "variants"}
    rust = {"None_" if v == "None" else v for v in e.variants()}
    assert members == rust


def test_every_vocabulary_enum_is_exported():
    assert set(ENUMS) >= {
        "Tag", "Scale", "Arithmetic", "FuncForm", "Construct", "Alt", "Kind", "Rule", "Code",
        "Dollar", "Backslash", "Join", "Quote", "Caret", "Tail", "Keywords", "Operators",
        "Devices", "Statement", "Analysis", "Command", "Simulator", "Writer", "NumberReading",
        "ModelSemantics", "SourceRules", "CardRules", "NamedRules", "OptionsForm", "LoadForm",
        "ControlBlock", "AnalysisForm", "EndRule", "ValueQuoting", "Coupling", "ParamForm",
        "NamedParams", "IncludeScope", "ExprEval", "Parasitics", "NetlistRules",
    }
    assert ps.def_.Keyword is Tag


NOISE = "* t\nV1 in 0 1 AC 1\nR1 in out 1k\nR2 out 0 1k\n.noise v(out) V1 dec 10 1 1meg\n.noise v(in) V1 dec 5 1 1k\n.end\n"


def test_a_python_hook_is_called_once_per_deck_and_spliced(acme):
    before = len(CALLS)
    text = ps.Testbench.from_source(NOISE, dialect="ngspice").netlist(acme)
    assert [l for l in text.splitlines() if "noise" in l] == [
        ".noise_acme out=V(out) src=V1 dec 10 1 1meg",
        ".noise_acme out=V(in) src=V1 dec 5 1 1k",
    ]
    assert CALLS[before:] == [2]


def test_a_hook_refusal_raises_with_its_code(acme):
    tb = ps.Testbench.from_source("* t\nV2 in 0 1 AC 1\nR1 in 0 1k\n.noise v(in) V2 dec 10 1 1k\n.end\n")
    with pytest.raises(RuntimeError, match=rf"W0201 {acme}:ACME002 acme's .noise takes one source"):
        tb.netlist(acme)


def test_a_refused_construct_raises_with_its_code(acme):
    tb = ps.Testbench.from_source(
        "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s*1e-3))\nR1 out 0 1k\n.op\n.end\n")
    with pytest.raises(RuntimeError, match=rf":3:1: W0201 {acme}:ACME001 acme has no Laplace sources"):
        tb.netlist(acme)


def test_a_plugin_targets_note_is_a_python_warning(uniq):
    """A plugin's Note-only omit (`omit_code` of a `Kind.Note` code) leaves
    the construct out and writes the deck; the note reaches Python as a
    `UserWarning` naming W0202 and the definition's id, as a built-in
    target's notes do (ZERO-LEDGER P1-038, P1-028)."""
    class Quiet(ps.dialects.NGSPICE):
        def define(self, d):
            return (super().define(d)
                    .code("Q001", Kind.Note, "quiet drops Laplace sources")
                    .omit_code(Construct.Laplace, "Q001"))

    quiet = ps.register_dialect(Quiet(uniq("quiet"))).name()
    tb = ps.Testbench.from_source(
        "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s*1e-3))\nR1 out 0 1k\n.op\n.end\n")
    with pytest.warns(UserWarning, match=rf"W0202 {quiet}:Q001 quiet drops Laplace sources"):
        text = tb.netlist(quiet)
    assert "laplace" not in text.lower()


def test_acme_text_reads_and_writes(acme):
    own = "# t\n.subcircuit load n PARAMS: r=1k ; a tail\nR1 n 0 {r}\n.endsub load\nV1 in 0 2x\nX1 in load r=2k\n.op\n.end\n"
    ng = ps.Testbench.from_source(own, dialect=acme).netlist("ngspice")
    assert ".subckt load n PARAMS: r=1k" in ng and "2meg" in ng
    back = ps.Testbench.from_source(ng, dialect="ngspice").netlist(acme)
    assert ".subcircuit load n PARAMS: r=1k" in back and ".endsub load" in back


def test_a_netlist_only_plugin_does_not_run(acme):
    tb = ps.Testbench.from_source("* t\nV1 in 0 1\nR1 in 0 1k\n.end\n")
    tb.add_operating_point()
    tb.with_backend(acme)
    with pytest.raises(RuntimeError, match=f"no simulator runs '{acme}'"):
        tb.run()


@pytest.mark.needs("ngspice")
def test_a_plugin_that_runs_on_ngspice_gives_ngspice_results(uniq):
    class MyNgspice(ps.dialects.NGSPICE):
        def define(self, d):
            return super().define(d).comment_tail("//").runs_on(Simulator.Ngspice)

    mine = uniq("my-ngspice")
    lang = ps.register_dialect(MyNgspice(mine))
    assert lang.runs_on() == Simulator.Ngspice

    def op(backend):
        dut = ps.Subcircuit("div", ["a", "b"])
        dut.R(name="1", positive="a", negative="b", value=1e3)
        dut.R(name="2", positive="b", negative="0", value=3e3)
        tb = ps.Testbench(dut)
        tb.V(name="s", positive="a", negative="0", value=2.0)
        tb.add_operating_point()
        tb.with_backend(backend)
        (result,) = tb.run()
        return result["b"]

    assert op(mine) == op("ngspice") == pytest.approx(1.5, abs=1e-9)
