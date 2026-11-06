"""The Python dialect-definition API (`spicerack.def_`, `spicerack.dialects`;
DIALECT-PLUGIN.md §2, §4, §5).

Oracles: the Rust rows' pinned dumps (`tests/fixtures/dialects/*.dump`,
written by `tests/test_dialect_plugin.rs` from the same calls in Rust):
acme and `MyHspice` declared here register rows whose `dump()` equals the
Rust ones byte for byte (oracle 3); the Rust variants' names (`variants()`,
from Rust's `Debug`) against each Python class's members (oracle 9); the
catalogue's `W0201` message with the definition's id; ngspice itself for a
plugin that runs on it (the same operating point as the ngspice deck).
"""
from pathlib import Path

import pytest

import spicerack as ps
from spicerack.def_ import (Alt, Answer, Arithmetic, Construct, FuncForm, Kind, Lang,
                            NumberReading, Scale, Simulator, Tag)

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


def acme():
    return ps.register_dialect(Acme("acme"))


def test_acme_dumps_the_rust_row():
    assert acme().dump() == (PINS / "acme.dump").read_text()


def test_my_hspice_dumps_the_rust_extends():
    assert ps.register_dialect(MyHspice("my-hspice")).dump() == (PINS / "my-hspice.dump").read_text()


def test_registration_is_idempotent_and_refuses_another_definition():
    assert acme() == acme()

    class Other(Acme):
        def define(self, d):
            return super().define(d).scale("y", Scale.Exp(3))

    with pytest.raises(ValueError, match="registered with another definition"):
        ps.register_dialect(Other("acme"))


def test_class_attributes_are_one_argument_calls():
    class Sugar(ps.Dialect):
        extends = Lang.NGSPICE
        fold_case = False

    sugar = ps.register_dialect(Sugar("sugar"))
    want = ps.def_.register(ps.def_.DialectDef("sugar-calls").extends(Lang.NGSPICE).fold_case(False))
    assert sugar.dump().replace("sugar", "X") == want.dump().replace("sugar-calls", "X")


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
        "NamedParams",
    }
    assert ps.def_.Keyword is Tag


NOISE = "* t\nV1 in 0 1 AC 1\nR1 in out 1k\nR2 out 0 1k\n.noise v(out) V1 dec 10 1 1meg\n.noise v(in) V1 dec 5 1 1k\n.end\n"


def test_a_python_hook_is_called_once_per_deck_and_spliced():
    acme()
    before = len(CALLS)
    text = ps.Testbench.from_source(NOISE, dialect="ngspice").netlist("acme")
    assert [l for l in text.splitlines() if "noise" in l] == [
        ".noise_acme out=V(out) src=V1 dec 10 1 1meg",
        ".noise_acme out=V(in) src=V1 dec 5 1 1k",
    ]
    assert CALLS[before:] == [2]


def test_a_hook_refusal_raises_with_its_code():
    acme()
    tb = ps.Testbench.from_source("* t\nV2 in 0 1 AC 1\nR1 in 0 1k\n.noise v(in) V2 dec 10 1 1k\n.end\n")
    with pytest.raises(RuntimeError, match=r"W0201 acme:ACME002 acme's .noise takes one source"):
        tb.netlist("acme")


def test_a_refused_construct_raises_with_its_code():
    acme()
    tb = ps.Testbench.from_source(
        "* t\nV1 in 0 1\nB1 out 0 V=Laplace(v(in), 1/(1+s*1e-3))\nR1 out 0 1k\n.op\n.end\n")
    with pytest.raises(RuntimeError, match=r":3:1: W0201 acme:ACME001 acme has no Laplace sources"):
        tb.netlist("acme")


def test_acme_text_reads_and_writes():
    acme()
    own = "# t\n.subcircuit load n PARAMS: r=1k ; a tail\nR1 n 0 {r}\n.endsub load\nV1 in 0 2x\nX1 in load r=2k\n.op\n.end\n"
    ng = ps.Testbench.from_source(own, dialect="acme").netlist("ngspice")
    assert ".subckt load n PARAMS: r=1k" in ng and "2meg" in ng
    back = ps.Testbench.from_source(ng, dialect="ngspice").netlist("acme")
    assert ".subcircuit load n PARAMS: r=1k" in back and ".endsub load" in back


def test_a_netlist_only_plugin_does_not_run():
    acme()
    tb = ps.Testbench.from_source("* t\nV1 in 0 1\nR1 in 0 1k\n.end\n")
    tb.add_operating_point()
    tb.with_backend("acme")
    with pytest.raises(RuntimeError, match="no simulator runs 'acme'"):
        tb.run()


@pytest.mark.needs("ngspice")
def test_a_plugin_that_runs_on_ngspice_gives_ngspice_results():
    class MyNgspice(ps.dialects.NGSPICE):
        def define(self, d):
            return super().define(d).comment_tail("//").runs_on(Simulator.Ngspice)

    lang = ps.register_dialect(MyNgspice("my-ngspice"))
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

    assert op("my-ngspice") == op("ngspice") == pytest.approx(1.5, abs=1e-9)
