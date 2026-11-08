"""Every built-in dialect declared again from Python, with the
`spicerack.def_` calls its Rust file (`src/syntax/dialects/<d>.rs`) makes,
in the same order: oracle 3 of DIALECT-PLUGIN.md §8, the Python parity of
the built-ins (`test_dialect_def.py` registers each and compares its dump
with the Rust row's). Written out call by call, translated once from the
Rust files, never generated from a dump; a change to a built-in's Rust
definition is made here too, or the parity test fails.

Each function takes the `DialectDef` a `Dialect` subclass's `define` is
given and returns it.
"""
from spicerack.def_ import (Alt, Analysis, AnalysisForm, Arithmetic, Backslash, CardRules, Caret,
                            Code, Command, Construct, ControlBlock, Coupling, Devices, Dollar, End,
                            EndRule, ExprEval, FuncForm, IncludeScope, Join, Keywords, Lang, NetlistRules,
                            LoadForm, ModelSemantics, NamedParams, NamedRules, NumberReading,
                            Operators, OptionsForm, ParamForm, Parasitics, Quote, Quoting, Rule,
                            Scale, SourceRules, Statement, Tag, Tail, ValueQuoting, Writer)


def ngspice(d):
    # §1.9: quotes protect comment bytes and take backslash escapes; an
    # unclosed one runs to the line's end or a `\\` break.
    quoted = Quoting(escapes=True, protects=True, across_breaks=False)
    d = d.title(True).fold_case(True)
    # §1.5: `*`, `#`, `$`, `;` are comments; the punctuation is turned into
    # one with a warning; so is a line of only a `//` comment.
    for s in ["*", "#", "$", ";", "=", "[", "]", "?", "(", ")", "&", "%", "\"", "!", ":", ",", "\x0c", "//",]:
        d = d.comment_line(s)
    return (d
        # §1.6: a `+` continues past `*`, `#`, `$` comment lines and blank lines.
        .cont_skip("*")
        .cont_skip("#")
        .cont_skip("$")
        .comment_tail(";")
        .comment_tail("//")
        .dollar(Dollar.AfterBlank)
        .plus(True)
        .backslash(Backslash.Glue)
        # §1.7: `.end` alone or before a blank is dropped, and reading goes on.
        .end(End(col1=False, then_blank=True, stops=False))
        .stop(ord("@"))
        .join(Join.None_)
        .quote(Quote.Expr(quoted))
        .string(quoted)
        .ident_start("$")
        .ident_continue("$.")
        .directive(ord("."))
        .operators(Operators.Spice)
        .expr_braces(True)
        .caret(Caret.Pow)
        # §2.2, numparam precedence (the old ngspice grammar x_ternary .. x_unary).
        .infix(Tag.PipePipe, 2)
        .infix(Tag.AmpAmp, 3)
        .infix(Tag.EqualEqual, 4)
        .infix(Tag.BangEqual, 4)
        .infix(Tag.LessGreater, 4)
        .infix(Tag.LessEqual, 4)
        .infix(Tag.GreaterEqual, 4)
        .infix(Tag.Less, 4)
        .infix(Tag.Greater, 4)
        .infix(Tag.Plus, 5)
        .infix(Tag.Minus, 5)
        .infix(Tag.Star, 6)
        .infix(Tag.Slash, 6)
        .infix(Tag.Percent, 6)
        .infix(Tag.Backslash, 6)
        .infix(Tag.Caret, 7)
        .infix(Tag.StarStar, 7)
        .prefix(Tag.Plus)
        .prefix(Tag.Minus)
        .prefix(Tag.Bang)
        .prefix_bp(6)
        .probe("v")
        .probe("i")
        .lib_sections(True)
        # §2.1.3: the scale factors; `D` an exponent.
        .exponent("eEdD")
        .scale("meg", Scale.Exp(6))
        .scale("mil", Scale.Mil)
        .scale("t", Scale.Exp(12))
        .scale("g", Scale.Exp(9))
        .scale("k", Scale.Exp(3))
        .scale("m", Scale.Exp(-3))
        .scale("u", Scale.Exp(-6))
        .scale("n", Scale.Exp(-9))
        .scale("p", Scale.Exp(-12))
        .scale("f", Scale.Exp(-15))
        .scale("a", Scale.Exp(-18))
        .tail(Tail.Spice(dot=True))
        .arithmetic(Arithmetic.Ngspice)
        .keywords(Keywords.Spice)
        .keyword(".control", Tag.KwControl)
        .keyword(".endc", Tag.KwEndc)
        .keyword(".sp", Tag.KwSp)
        .devices(Devices.Spice)
        .devices(Devices.NgspiceOwn)
        # The spelling: §2.8.5 `{…}`, §2.4.3 `PARAMS:`.
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm.Func)
        .subckt_params("PARAMS:")
        .ngspice_numbers(True)
        # The behaviours.
        .number_reading(NumberReading.Ngspice)
        .model_semantics(ModelSemantics.Ngspice)
        .source_rules(SourceRules.Ngspice)
        .card_rules(CardRules.Ngspice)
        .load_form(LoadForm.Ngspice)
        .control_block(ControlBlock.Ngspice)
        .analysis_form(AnalysisForm.Ngspice)
        .expr_eval(ExprEval.Ngspice)
        .netlist_rules(NetlistRules.Ngspice)
        .spectre_analysis("pz")
        .spectre_analysis("sp")
        .spectre_analysis("sens")
        # The circuit temperature in an expression (manual §2.8.5).
        .temp_name("temper")
        # ngspice-45's built-in devices (`devhelp`): a model type of one of
        # these names is the built-in, never a loaded OSDI module (W0121).
        .builtin_device("URC")
        .builtin_device("ASRC")
        .builtin_device("BJT")
        .builtin_device("BSIM1")
        .builtin_device("BSIM2")
        .builtin_device("BSIM3")
        .builtin_device("BSIM3v0")
        .builtin_device("BSIM3v1")
        .builtin_device("BSIM3v32")
        .builtin_device("B4SOI")
        .builtin_device("BSIM4")
        .builtin_device("BSIM4v5")
        .builtin_device("BSIM4v6")
        .builtin_device("BSIM4v7")
        .builtin_device("B3SOIPD")
        .builtin_device("B3SOIFD")
        .builtin_device("B3SOIDD")
        .builtin_device("Capacitor")
        .builtin_device("CCCS")
        .builtin_device("CCVS")
        .builtin_device("CplLines")
        .builtin_device("CSwitch")
        .builtin_device("Diode")
        .builtin_device("HFET1")
        .builtin_device("HFET2")
        .builtin_device("hicum2")
        .builtin_device("HiSIM2")
        .builtin_device("HiSIMHV1")
        .builtin_device("HiSIMHV2")
        .builtin_device("Inductor")
        .builtin_device("mutual")
        .builtin_device("Isource")
        .builtin_device("JFET")
        .builtin_device("JFET2")
        .builtin_device("LTRA")
        .builtin_device("MES")
        .builtin_device("MESA")
        .builtin_device("Mos1")
        .builtin_device("Mos2")
        .builtin_device("Mos3")
        .builtin_device("Mos6")
        .builtin_device("Mos9")
        .builtin_device("Resistor")
        .builtin_device("SOI3")
        .builtin_device("Switch")
        .builtin_device("Tranline")
        .builtin_device("TransLine")
        .builtin_device("VBIC")
        .builtin_device("VCCS")
        .builtin_device("VCVS")
        .builtin_device("Vsource")
        .builtin_device("VDMOS")
        .builtin_device("NBJT")
        .builtin_device("NBJT2")
        .builtin_device("NUMD")
        .builtin_device("NUMD2")
        .builtin_device("NUMOS")
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .refuse(Construct.XyceSens, Code.W0015)
        .refuse(Construct.HspiceAnalysis, Code.W0015)
        .refuse(Construct.HspiceWLine, Code.W0105)
        .omit(Construct.VacaskScript, Code.W0120)
        .refuse(Construct.VacaskElaborate, Code.W0122)
        .refuse(Construct.CaretReread, Code.W0002)
        .native(Construct.StatisticalCall)
        .spell(Construct.VerilogALoad, Alt.PreOsdi)
        .spell(Construct.OsdiLoad, Alt.PreOsdi)
        .native(Construct.OsdiInstance)
        .spell(Construct.LibWholeFile, Alt.Include)
        .spell(Construct.NgBehavior, Alt.NgBehaviorLine)
        .native(Construct.LibSection)
        .native(Construct.RclModel)
        .spell(Construct.ChargeCapacitor, Alt.ChargeQuoted)
        .native(Construct.BehaviouralSource)
        .lift(Construct.Laplace, Rule.LaplaceXfer)
        .native(Construct.RfPort)
        .native(Construct.PwlRepeatV)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .native(Construct.NoiseWave)
        .native(Construct.Conditional)
        .refuse(Construct.BranchParam, Code.W0014)
        .native(Construct.Func)
        .native(Construct.LocalFunc)
        .spell(Construct.TableSource, Alt.PwlValue)
        .native(Construct.LossyLine)
        .native(Construct.AmWave)
        .refuse(Construct.Step, Code.W0016)
        .refuse(Construct.StepList, Code.W0016)
        .refuse(Construct.TranStartup, Code.W0037)
        .refuse(Construct.DcLogList, Code.W0038)
        .native(Construct.Tf)
        .native(Construct.Sens)
        .native(Construct.SensAc)
        .native(Construct.PoleZero)
        .native(Construct.SParameter)
        .native(Construct.Print)
        .native(Construct.Save)
        .native(Construct.UnmappedOption)
        .lift(Construct.Statistics, Rule.StatisticsDraws)
        .native(Construct.Measure))


def espice(d):
    return (d
        .extends(Lang.NGSPICE)
        .text_family(Lang.NGSPICE)
        # The behaviours: ESPice is never read as itself (its text is
        # ngspice's), so its reading rows are the plain SPICE ones.
        .number_reading(NumberReading.Decimal)
        .model_semantics(ModelSemantics.Other)
        .source_rules(SourceRules.Spice)
        .card_rules(CardRules.Spice)
        .load_form(LoadForm.Espice)
        .control_block(ControlBlock.None_)
        .analysis_form(AnalysisForm.Espice)
        .subckt_multiplicity(False)
        # The capability cells it answers otherwise than ngspice.
        .spell(Construct.VerilogALoad, Alt.Hdl)
        .refuse(Construct.OsdiLoad, Code.W0004)
        .refuse(Construct.OsdiInstance, Code.W0004)
        .omit(Construct.NgBehavior, Code.W0030)
        .lift(Construct.Laplace, Rule.LaplaceTable)
        .native(Construct.PwlRepeatI)
        .refuse(Construct.NoiseWave, Code.W0009)
        .refuse(Construct.TableSource, Code.W0102)
        # ESPice 0.3.0 runs ngspice's AM but no VO: "AM with a nonzero VO
        # offset" (run).
        .spell(Construct.AmWave, Alt.AmNoOffset))


HSPICE_QUOTED = Quoting(escapes=False,
    protects=True,
    across_breaks=True)

HSPICE_OPERATORS = [
    ("**", Tag.StarStar),
    ("==", Tag.EqualEqual),
    ("!=", Tag.BangEqual),
    ("<=", Tag.LessEqual),
    (">=", Tag.GreaterEqual),
    ("&&", Tag.AmpAmp),
    ("||", Tag.PipePipe),
]

HSPICE_SCALES = [
    ("meg", Scale.Exp(6)),
    ("x", Scale.Exp(6)),
    ("mil", Scale.Mil),
    ("t", Scale.Exp(12)),
    ("g", Scale.Exp(9)),
    ("k", Scale.Exp(3)),
    ("m", Scale.Exp(-3)),
    ("u", Scale.Exp(-6)),
    ("n", Scale.Exp(-9)),
    ("p", Scale.Exp(-12)),
    ("f", Scale.Exp(-15)),
    ("a", Scale.Exp(-18)),
]

HSPICE_OWN_KEYWORDS = [(".probe", Tag.KwProbe)]

def hspice(d):
    d = d.title(True)
    d = d.fold_case(True)
    d = d.comment_line("*")
    d = d.comment_line("$")
    d = d.cont_skip("*")
    d = d.cont_skip("$")
    d = d.dollar(Dollar.Hspice)
    d = d.plus(True)
    d = d.backslash(Backslash.Hspice)
    # L9: `.END` ends the run; what follows is a further run, not read.
    d = d.end(End(col1=False, then_blank=False, stops=True))
    d = d.stop(None)
    d = d.join(Join.None_)
    d = d.quote(Quote.Expr(HSPICE_QUOTED))
    d = d.string(HSPICE_QUOTED)
    d = d.string_expr(True)
    d = d.ident_start("$")
    d = d.ident_continue("$.")
    d = d.directive(ord("."))
    for s, t in HSPICE_OPERATORS:
        d = d.operator(s, t)
    d = d.caret(Caret.Pow)
    # X2 (UG08 pp.279-283, Table 19): no `%`, `\\` or unary `!`.
    d = d.infix(Tag.PipePipe, 2)
    d = d.infix(Tag.AmpAmp, 3)
    d = d.infix(Tag.EqualEqual, 4)
    d = d.infix(Tag.BangEqual, 4)
    d = d.infix(Tag.LessEqual, 4)
    d = d.infix(Tag.GreaterEqual, 4)
    d = d.infix(Tag.Less, 4)
    d = d.infix(Tag.Greater, 4)
    d = d.infix(Tag.Plus, 5)
    d = d.infix(Tag.Minus, 5)
    d = d.infix(Tag.Star, 6)
    d = d.infix(Tag.Slash, 6)
    d = d.infix(Tag.Caret, 7)
    d = d.infix(Tag.StarStar, 7)
    d = d.prefix(Tag.Plus)
    d = d.prefix(Tag.Minus)
    d = d.prefix_bp(6)
    d = d.probe("v")
    d = d.probe("i")
    d = d.lib_sections(True)
    d = d.exponent("eEdD")
    for s, k in HSPICE_SCALES:
        d = d.scale(s, k)
    d = d.tail(Tail.Spice(dot=True))
    d = d.arithmetic(Arithmetic.Decimal)
    d = d.keywords(Keywords.Spice)
    for s, t in HSPICE_OWN_KEYWORDS:
        d = d.keyword(s, t)
    d = d.devices(Devices.Spice)
    return (d
        # The spelling.
        .comment("*")
        .path_quote("'")
        .expr("'", "'")
        .func(FuncForm.Param)
        .subckt_params(None)
        # The behaviours.
        .card_rules(CardRules.Hspice)
        .load_form(LoadForm.Hspice)
        .analysis_form(AnalysisForm.Hspice)
        .end_rule(EndRule.WhenRuns)
        .value_quoting(ValueQuoting.Hspice)
        .expr_eval(ExprEval.Hspice)
        .netlist_rules(NetlistRules.Hspice)
        .spectre_analysis("pz")
        .spectre_analysis("sens")
        .spectre_analysis("sp")
        .spectre_analysis("hb")
        .spectre_analysis("sweep")
        .spectre_analysis("montecarlo")
        # The circuit temperature in an expression (HSPICE UG, temper).
        .temp_name("temper")
        # The VACASK analyses it has (HSPICE RF .HB, .HBAC, .HBNOISE).
        .vacask_analysis("hb")
        .vacask_analysis("hbac")
        .vacask_analysis("hbnoise")
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .refuse(Construct.XyceSens, Code.W0015)
        .native(Construct.HspiceAnalysis)
        .native(Construct.HspiceWLine)
        .omit(Construct.VacaskScript, Code.W0120)
        .refuse(Construct.VacaskElaborate, Code.W0122)
        .refuse(Construct.CaretReread, Code.W0002)
        .native(Construct.StatisticalCall)
        .spell(Construct.VerilogALoad, Alt.Hdl)
        .refuse(Construct.OsdiLoad, Code.W0004)
        .refuse(Construct.OsdiInstance, Code.W0004)
        .spell(Construct.LibWholeFile, Alt.Include)
        .omit(Construct.NgBehavior, Code.W0030)
        .native(Construct.LibSection)
        .native(Construct.RclModel)
        .spell(Construct.ChargeCapacitor, Alt.ChargeQuoted)
        .spell(Construct.BehaviouralSource, Alt.VolCur)
        .lift(Construct.Laplace, Rule.LaplaceTable)
        .refuse(Construct.RfPort, Code.W0007)
        .refuse(Construct.PwlRepeatV, Code.W0008)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .refuse(Construct.NoiseWave, Code.W0009)
        .native(Construct.Conditional)
        .refuse(Construct.BranchParam, Code.W0014)
        .spell(Construct.Func, Alt.FuncParam)
        .native(Construct.LocalFunc)
        .refuse(Construct.TableSource, Code.W0102)
        .refuse(Construct.LossyLine, Code.W0105)
        .spell(Construct.AmWave, Alt.HspiceAm)
        .refuse(Construct.Step, Code.W0016)
        .spell(Construct.StepList, Alt.TempList)
        .refuse(Construct.TranStartup, Code.W0037)
        .spell(Construct.DcLogList, Alt.DcType)
        .native(Construct.Tf)
        .native(Construct.Sens)
        .refuse(Construct.SensAc, Code.W0015)
        # HSPICE UG `.PZ out src`: where a source stands across the input.
        .spell(Construct.PoleZero, Alt.HspicePz)
        .refuse(Construct.SParameter, Code.W0015)
        .native(Construct.Print)
        .spell(Construct.Save, Alt.Probe)
        .native(Construct.UnmappedOption)
        .lift(Construct.Statistics, Rule.StatisticsDraws)
        .native(Construct.Measure))


XYCE_UNPROTECTED = Quoting(escapes=False,
    protects=False,
    across_breaks=False)

XYCE_SCALES = [
    ("meg", Scale.Exp(6)),
    ("mil", Scale.Mil),
    ("x", Scale.Exp(6)),
    ("t", Scale.Exp(12)),
    ("g", Scale.Exp(9)),
    ("k", Scale.Exp(3)),
    ("m", Scale.Exp(-3)),
    ("u", Scale.Exp(-6)),
    ("n", Scale.Exp(-9)),
    ("p", Scale.Exp(-12)),
    ("f", Scale.Exp(-15)),
]

XYCE_OWN_KEYWORDS = [
    (".incl", Tag.KwInclude),
    (".step", Tag.KwStep),
    (".global_param", Tag.KwParam),
]

def xyce(d):
    d = d.title(True)
    d = d.fold_case(True)
    d = d.comment_line("*")
    d = d.blank_led_comment(True)
    d = d.cont_skip("*")
    d = d.comment_tail(";")
    d = d.dollar(Dollar.Text)
    d = d.plus(True)
    d = d.backslash(Backslash.None_)
    d = d.end(End(col1=True, then_blank=True, stops=True))
    d = d.stop(None)
    d = d.join(Join.None_)
    d = d.quote(Quote.Expr(XYCE_UNPROTECTED))
    d = d.string(XYCE_UNPROTECTED)
    d = d.ident_start("$")
    d = d.ident_continue("$.:")
    d = d.directive(ord("."))
    d = d.operators(Operators.Spice)
    d = d.expr_braces(True)
    d = d.caret(Caret.Xor)
    # Table 2-32 (RG p.164), the levels measured on Xyce 7.10.
    d = d.infix(Tag.PipePipe, 2)
    d = d.infix(Tag.Pipe, 2)
    d = d.infix(Tag.AmpAmp, 2)
    d = d.infix(Tag.Amp, 2)
    d = d.infix(Tag.Caret, 3)
    d = d.infix(Tag.EqualEqual, 4)
    d = d.infix(Tag.BangEqual, 4)
    d = d.infix(Tag.LessGreater, 4)
    d = d.infix(Tag.LessEqual, 4)
    d = d.infix(Tag.GreaterEqual, 4)
    d = d.infix(Tag.Less, 4)
    d = d.infix(Tag.Greater, 4)
    d = d.infix(Tag.Plus, 5)
    d = d.infix(Tag.Minus, 5)
    d = d.infix(Tag.Percent, 6)
    d = d.infix(Tag.Star, 7)
    d = d.infix(Tag.Slash, 7)
    d = d.infix(Tag.StarStar, 9)
    d = d.right(Tag.StarStar)
    d = d.prefix(Tag.Plus)
    d = d.prefix(Tag.Minus)
    d = d.prefix(Tag.Tilde)
    d = d.prefix_bp(8)
    # RG §2.1.31 pp.130-133.
    d = d.probe("v")
    d = d.probe("vr")
    d = d.probe("vi")
    d = d.probe("vm")
    d = d.probe("vp")
    d = d.probe("vdb")
    d = d.probe("i")
    d = d.probe("ir")
    d = d.probe("ii")
    d = d.probe("im")
    d = d.probe("ip")
    d = d.probe("idb")
    d = d.probe("n")
    d = d.probe("p")
    d = d.probe("w")
    d = d.lib_sections(True)
    d = d.exponent("eE")
    for s, k in XYCE_SCALES:
        d = d.scale(s, k)
    d = d.tail(Tail.Spice(dot=False))
    d = d.arithmetic(Arithmetic.Decimal)
    d = d.keywords(Keywords.Spice)
    for s, t in XYCE_OWN_KEYWORDS:
        d = d.keyword(s, t)
    d = d.devices(Devices.Spice)
    d = d.devices(Devices.LossyLine)
    d = d.devices(Devices.XycePort)
    return (d
        # The spelling.
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm.Func)
        .subckt_params("PARAMS:")
        # The behaviours.
        .caret_power(False)
        .hier_sep(":")
        .card_rules(CardRules.Xyce)
        .options_form(OptionsForm.XycePackages)
        .load_form(LoadForm.Xyce)
        .analysis_form(AnalysisForm.Xyce)
        .coupling(Coupling.OneCard)
        .param_form(ParamForm.Xyce)
        .expr_eval(ExprEval.Xyce)
        .netlist_rules(NetlistRules.Xyce)
        .include_scope(IncludeScope.Global)
        .spectre_analysis("hb")
        .spectre_analysis("sp")
        .spectre_analysis("sens")
        .spectre_analysis("sweep")
        .spectre_analysis("montecarlo")
        # The circuit temperature in an expression (RG, special variable TEMP).
        .temp_name("temp")
        # The VACASK analyses it has (Xyce RG .HB).
        .vacask_analysis("hb")
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .native(Construct.XyceSens)
        .refuse(Construct.HspiceAnalysis, Code.W0015)
        .refuse(Construct.HspiceWLine, Code.W0105)
        .omit(Construct.VacaskScript, Code.W0120)
        .refuse(Construct.VacaskElaborate, Code.W0122)
        .refuse(Construct.CaretReread, Code.W0002)
        .native(Construct.StatisticalCall)
        .refuse(Construct.VerilogALoad, Code.W0004)
        .refuse(Construct.OsdiLoad, Code.W0004)
        .refuse(Construct.OsdiInstance, Code.W0004)
        .spell(Construct.LibWholeFile, Alt.Include)
        .omit(Construct.NgBehavior, Code.W0030)
        .native(Construct.LibSection)
        .native(Construct.RclModel)
        .spell(Construct.ChargeCapacitor, Alt.ChargeBraced)
        .spell(Construct.BehaviouralSource, Alt.BracedBehaviour)
        .refuse(Construct.Laplace, Code.W0011)
        .spell(Construct.RfPort, Alt.XycePort)
        .refuse(Construct.PwlRepeatV, Code.W0008)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .refuse(Construct.NoiseWave, Code.W0009)
        .refuse(Construct.Conditional, Code.W0013)
        .refuse(Construct.BranchParam, Code.W0014)
        .native(Construct.Func)
        .native(Construct.LocalFunc)
        .native(Construct.TableSource)
        .native(Construct.LossyLine)
        .refuse(Construct.AmWave, Code.W0108)
        .native(Construct.Step)
        .native(Construct.StepList)
        .refuse(Construct.TranStartup, Code.W0037)
        .native(Construct.DcLogList)
        .refuse(Construct.Tf, Code.W0015)
        .refuse(Construct.Sens, Code.W0015)
        .refuse(Construct.SensAc, Code.W0015)
        .refuse(Construct.PoleZero, Code.W0015)
        .spell(Construct.SParameter, Alt.XyceLin)
        .native(Construct.Print)
        .omit(Construct.Save, Code.W0023)
        .refuse(Construct.UnmappedOption, Code.W0017)
        .lift(Construct.Statistics, Rule.StatisticsDraws)
        .native(Construct.Measure))


LTSPICE_OPERATORS = [
    ("**", Tag.StarStar),
    ("==", Tag.EqualEqual),
    ("!=", Tag.BangEqual),
    ("<=", Tag.LessEqual),
    (">=", Tag.GreaterEqual),
]

LTSPICE_SCALES = [
    ("meg", Scale.Exp(6)),
    ("mil", Scale.Mil),
    ("µ", Scale.Exp(-6)),
    ("t", Scale.Exp(12)),
    ("g", Scale.Exp(9)),
    ("k", Scale.Exp(3)),
    ("m", Scale.Exp(-3)),
    ("u", Scale.Exp(-6)),
    ("n", Scale.Exp(-9)),
    ("p", Scale.Exp(-12)),
    ("f", Scale.Exp(-15)),
]

LTSPICE_OWN_KEYWORDS = [
    (".params", Tag.KwParam),
    (".step", Tag.KwStep),
    (".fourier", Tag.Directive),
]

def ltspice(d):
    d = d.title(True)
    d = d.fold_case(True)
    d = d.comment_line("*")
    d = d.comment_line(";")
    d = d.cont_skip("*")
    d = d.cont_skip(";")
    d = d.comment_tail(";")
    d = d.dollar(Dollar.Text)
    d = d.plus(True)
    d = d.backslash(Backslash.None_)
    d = d.end(End(col1=False, then_blank=False, stops=True))
    d = d.stop(None)
    d = d.join(Join.None_)
    d = d.quote(Quote.Plain)
    # A `;` inside double quotes (a path) is no comment (unverified).
    d = d.string(Quoting(escapes=False, protects=True, across_breaks=False))
    d = d.ident_start("$")
    d = d.ident_continue("$.")
    d = d.directive(ord("."))
    for s, t in LTSPICE_OPERATORS:
        d = d.operator(s, t)
    d = d.expr_braces(True)
    d = d.caret(Caret.Xor)
    # CER "Supported Operators (ascending precedence)": `& | ^`,
    # comparisons, `+ -`, `* / %`, `**`, `! ~`.
    d = d.infix(Tag.Amp, 2)
    d = d.infix(Tag.Pipe, 2)
    d = d.infix(Tag.Caret, 2)
    d = d.infix(Tag.EqualEqual, 3)
    d = d.infix(Tag.BangEqual, 3)
    d = d.infix(Tag.LessEqual, 3)
    d = d.infix(Tag.GreaterEqual, 3)
    d = d.infix(Tag.Less, 3)
    d = d.infix(Tag.Greater, 3)
    d = d.infix(Tag.Plus, 4)
    d = d.infix(Tag.Minus, 4)
    d = d.infix(Tag.Star, 5)
    d = d.infix(Tag.Slash, 5)
    d = d.infix(Tag.Percent, 5)
    d = d.infix(Tag.StarStar, 6)
    d = d.prefix(Tag.Plus)
    d = d.prefix(Tag.Minus)
    d = d.prefix(Tag.Bang)
    d = d.prefix(Tag.Tilde)
    d = d.prefix_bp(7)
    d = d.probe("v")
    d = d.probe("i")
    d = d.exponent("eE")
    for s, k in LTSPICE_SCALES:
        d = d.scale(s, k)
    d = d.tail(Tail.Ltspice)
    d = d.arithmetic(Arithmetic.Decimal)
    # `.fourier` is LTspice's own card, not `.four`.
    for s, t in LTSPICE_OWN_KEYWORDS:
        d = d.keyword(s, t)
    d = d.keywords(Keywords.Spice)
    d = d.devices(Devices.Spice)
    d = d.devices(Devices.LossyLine)
    return (d
        # The spelling.
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm.Func)
        .subckt_params("PARAMS:")
        # The behaviours.
        .caret_power(False)
        .card_rules(CardRules.Ltspice)
        .load_form(LoadForm.Ltspice)
        .analysis_form(AnalysisForm.Ltspice)
        .expr_eval(ExprEval.Ltspice)
        .netlist_rules(NetlistRules.Ltspice)
        .parasitics(Parasitics.Ltspice)
        .coupling(Coupling.OneCard)
        .spectre_analysis("sweep")
        # The circuit temperature in an expression (help, temp).
        .temp_name("temp")
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .refuse(Construct.XyceSens, Code.W0015)
        .refuse(Construct.HspiceAnalysis, Code.W0015)
        .refuse(Construct.HspiceWLine, Code.W0105)
        .omit(Construct.VacaskScript, Code.W0120)
        .refuse(Construct.VacaskElaborate, Code.W0122)
        .refuse(Construct.CaretReread, Code.W0002)
        .refuse(Construct.StatisticalCall, Code.W0003)
        .refuse(Construct.VerilogALoad, Code.W0004)
        .refuse(Construct.OsdiLoad, Code.W0004)
        .refuse(Construct.OsdiInstance, Code.W0004)
        .native(Construct.LibWholeFile)
        .omit(Construct.NgBehavior, Code.W0030)
        .refuse(Construct.LibSection, Code.W0012)
        .refuse(Construct.RclModel, Code.W0006)
        .spell(Construct.ChargeCapacitor, Alt.ChargeBare)
        .native(Construct.BehaviouralSource)
        .lift(Construct.Laplace, Rule.LaplaceAttr)
        .refuse(Construct.RfPort, Code.W0007)
        .refuse(Construct.PwlRepeatV, Code.W0008)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .refuse(Construct.NoiseWave, Code.W0009)
        .refuse(Construct.Conditional, Code.W0013)
        .refuse(Construct.BranchParam, Code.W0014)
        .native(Construct.Func)
        .refuse(Construct.LocalFunc, Code.W0106)
        .refuse(Construct.TableSource, Code.W0102)
        .native(Construct.LossyLine)
        .refuse(Construct.AmWave, Code.W0108)
        .native(Construct.Step)
        .native(Construct.StepList)
        .native(Construct.TranStartup)
        .native(Construct.DcLogList)
        .native(Construct.Tf)
        .refuse(Construct.Sens, Code.W0015)
        .refuse(Construct.SensAc, Code.W0015)
        .refuse(Construct.PoleZero, Code.W0015)
        .refuse(Construct.SParameter, Code.W0015)
        .omit(Construct.Print, Code.W0023)
        .native(Construct.Save)
        .native(Construct.UnmappedOption)
        .lift(Construct.Statistics, Rule.StatisticsDraws)
        .native(Construct.Measure))


SPECTRE_SCALES = [
    ("P", Scale.Exp(15)),
    ("T", Scale.Exp(12)),
    ("G", Scale.Exp(9)),
    ("M", Scale.Exp(6)),
    # `k` first: the writer spells kilo with the first (Cadence's own
    # examples write `1k`).
    ("k", Scale.Exp(3)),
    ("K", Scale.Exp(3)),
    ("_", Scale.Exp(0)),
    ("%", Scale.Exp(-2)),
    ("c", Scale.Exp(-2)),
    ("m", Scale.Exp(-3)),
    ("u", Scale.Exp(-6)),
    ("n", Scale.Exp(-9)),
    ("p", Scale.Exp(-12)),
    ("f", Scale.Exp(-15)),
    ("a", Scale.Exp(-18)),
]

SPECTRE_KEYWORDS = [
    ("simulator", Tag.KwSimulator),
    ("include", Tag.KwInclude),
    ("ahdl_include", Tag.KwAhdlInclude),
    ("library", Tag.KwLibrary),
    ("endlibrary", Tag.KwEndlibrary),
    ("section", Tag.KwSection),
    ("endsection", Tag.KwEndsection),
    ("inline", Tag.KwInline),
    ("subckt", Tag.KwSubckt),
    ("ends", Tag.KwEnds),
    ("model", Tag.KwModel),
    ("parameters", Tag.KwParam),
    ("statistics", Tag.KwStatistics),
    ("process", Tag.KwProcess),
    ("mismatch", Tag.KwMismatch),
    ("vary", Tag.KwVary),
    ("correlate", Tag.KwCorrelate),
    ("truncate", Tag.KwTruncate),
    ("if", Tag.KwIf),
    ("else", Tag.KwElse),
    ("real", Tag.KwReal),
    ("return", Tag.KwReturn),
    ("global", Tag.KwGlobal),
    ("save", Tag.KwSave),
    ("ic", Tag.KwIc),
    ("nodeset", Tag.KwNodeset),
]

SPECTRE_MASTERS = [
        ("montecarlo", Statement.Analysis(Analysis.Montecarlo)),
        ("acmatch", Statement.Analysis(Analysis.Acmatch)),
        ("dcmatch", Statement.Analysis(Analysis.Dcmatch)),
        ("hbnoise", Statement.Analysis(Analysis.Hbnoise)),
        ("qpnoise", Statement.Analysis(Analysis.Qpnoise)),
        ("fourier", Statement.Analysis(Analysis.Fourier)),
        ("pdisto", Statement.Analysis(Analysis.Pdisto)),
        ("pnoise", Statement.Analysis(Analysis.Pnoise)),
        ("hbstb", Statement.Analysis(Analysis.Hbstb)),
        ("envlp", Statement.Analysis(Analysis.Envlp)),
        ("hbsp", Statement.Analysis(Analysis.Hbsp)),
        ("hbac", Statement.Analysis(Analysis.Hbac)),
        ("hbxf", Statement.Analysis(Analysis.Hbxf)),
        ("noise", Statement.Analysis(Analysis.Noise)),
        ("qpac", Statement.Analysis(Analysis.Qpac)),
        ("qpsp", Statement.Analysis(Analysis.Qpsp)),
        ("qpss", Statement.Analysis(Analysis.Qpss)),
        ("qpxf", Statement.Analysis(Analysis.Qpxf)),
        ("sweep", Statement.Analysis(Analysis.Sweep)),
        ("pac", Statement.Analysis(Analysis.Pac)),
        ("psp", Statement.Analysis(Analysis.Psp)),
        ("pss", Statement.Analysis(Analysis.Pss)),
        ("pstb", Statement.Analysis(Analysis.Pstb)),
        ("pxf", Statement.Analysis(Analysis.Pxf)),
        ("sens", Statement.Analysis(Analysis.Sens)),
        ("tran", Statement.Analysis(Analysis.Tran)),
        ("tdr", Statement.Analysis(Analysis.Tdr)),
        ("stb", Statement.Analysis(Analysis.Stb)),
        ("ac", Statement.Analysis(Analysis.Ac)),
        ("dc", Statement.Analysis(Analysis.Dc)),
        ("hb", Statement.Analysis(Analysis.Hb)),
        ("pz", Statement.Analysis(Analysis.Pz)),
        ("sp", Statement.Analysis(Analysis.Sp)),
        ("xf", Statement.Analysis(Analysis.Xf)),
        ("altergroup", Statement.Command(Command.Altergroup)),
        ("checklimit", Statement.Command(Command.Checklimit)),
        ("paramtest", Statement.Command(Command.Paramtest)),
        ("paramset", Statement.Command(Command.Paramset)),
        ("options", Statement.Options),
        ("assert", Statement.Command(Command.Assert)),
        ("alter", Statement.Command(Command.Alter)),
        ("check", Statement.Command(Command.Check)),
        ("shell", Statement.Command(Command.Shell)),
        ("info", Statement.Command(Command.Info)),
        ("set", Statement.Command(Command.Set)),
    ]

def spectre(d):
    d = d.title(True)
    d = d.comment_line("//")
    d = d.comment_col1(ord("*"))
    d = d.cont_skip("//")
    d = d.comment_tail("//")
    d = d.dollar(Dollar.Text)
    d = d.plus(True)
    d = d.backslash(Backslash.Join)
    d = d.end(None)
    d = d.stop(None)
    d = d.join(Join.Parens)
    d = d.quote(Quote.Plain)
    d = d.string(Quoting(escapes=True, protects=True, across_breaks=False))
    d = d.name_escape(True)
    d = d.ident_start("$")
    d = d.ident_continue("!$")
    d = d.directive(None)
    d = d.head_words(True)
    d = d.head_after(Tag.LBrace)
    d = d.head_after(Tag.RBrace)
    d = d.head_after(Tag.KwInline)
    d = d.head_after(Tag.KwElse)
    d = d.operator("**", Tag.StarStar)
    d = d.operator("==", Tag.EqualEqual)
    d = d.operator("!=", Tag.BangEqual)
    d = d.operator("<=", Tag.LessEqual)
    d = d.operator(">=", Tag.GreaterEqual)
    d = d.operator("&&", Tag.AmpAmp)
    d = d.operator("||", Tag.PipePipe)
    d = d.caret(Caret.None_)
    # §6.1 (UG p.87), decreasing: unary, `**` (right), `* / %`, `+ -`,
    # relational, equality, `&&`, `||`, `?:`.
    d = d.infix(Tag.PipePipe, 2)
    d = d.infix(Tag.AmpAmp, 3)
    d = d.infix(Tag.EqualEqual, 4)
    d = d.infix(Tag.BangEqual, 4)
    d = d.infix(Tag.LessEqual, 5)
    d = d.infix(Tag.GreaterEqual, 5)
    d = d.infix(Tag.Less, 5)
    d = d.infix(Tag.Greater, 5)
    d = d.infix(Tag.Plus, 6)
    d = d.infix(Tag.Minus, 6)
    d = d.infix(Tag.Star, 7)
    d = d.infix(Tag.Slash, 7)
    d = d.infix(Tag.Percent, 7)
    d = d.infix(Tag.StarStar, 8)
    d = d.right(Tag.StarStar)
    d = d.prefix(Tag.Plus)
    d = d.prefix(Tag.Minus)
    d = d.prefix(Tag.Bang)
    d = d.prefix_bp(9)
    d = d.lib_sections(True)
    d = d.exponent("eE")
    for s, k in SPECTRE_SCALES:
        d = d.scale(s, k)
    d = d.tail(Tail.Letters(after_exponent=True, hex=False))
    d = d.arithmetic(Arithmetic.Decimal)
    for s, t in SPECTRE_KEYWORDS:
        d = d.keyword(s, t)
    d = d.named(True)
    d = d.devices(Devices.Spectre)
    for s, m in SPECTRE_MASTERS:
        d = d.master(s, m)
    d = d.region("spice", Lang.HSPICE)
    return (d
        # The spelling.
        .comment("//")
        .path_quote("\"")
        .expr("", "")
        .func(FuncForm.Real)
        .subckt_params(None)
        # The behaviours.
        .writer(Writer.Spectre)
        .named_rules(NamedRules.Spectre)
        .options_form(OptionsForm.Spectre)
        .named_params(NamedParams.Spectre)
        .analysis_masters(Lang.SPECTRE)
        # SPICE's options and Spectre's of the same effect (`[REF]` 19.1
        # pp.187-236, by number): `reltol` #1, `vabstol` #3, `iabstol` #4,
        # `chargeabstol` #5, `temp` #6, `tnom` #7, `dcmaxiters` #56
        # (ngspice's `itl1`, Xyce's `max_iterations`), `scale` #90, `gmin`
        # #128, `pivrel` #181, `pivabs` #182 (SPICE's `pivtol`).
        .option_to("reltol", "reltol")
        .option_to("vntol", "vabstol")
        .option_to("abstol", "iabstol")
        .option_to("chgtol", "chargeabstol")
        .option_to("temp", "temp")
        .option_to("tnom", "tnom")
        .option_to("itl1", "dcmaxiters")
        .option_to("max_iterations", "dcmaxiters")
        .option_to("scale", "scale")
        .option_to("gmin", "gmin")
        .option_to("pivrel", "pivrel")
        .option_to("pivtol", "pivabs")
        # The master a SPICE `.model` type gets at a LEVEL (`[REF]` 19.1 p.498
        # `model name master type=`; `[CMP]` 5.0 mos1/mos2/mos3, bsim3v3, bsim4,
        # bjt, diode; ngspice-45 `inpdomod.c` LEVEL per type). A SPICE type with
        # no row at its LEVEL is refused (W0104); the line types and `res`,
        # `cap`, `ind` are none of Spectre's SPICE types.
        .model_master("nmos", [1], "mos1", "n")
        .model_master("nmos", [2], "mos2", "n")
        .model_master("nmos", [3], "mos3", "n")
        .model_master("nmos", [8, 49], "bsim3v3", "n")
        .model_master("nmos", [14, 54], "bsim4", "n")
        .model_master("pmos", [1], "mos1", "p")
        .model_master("pmos", [2], "mos2", "p")
        .model_master("pmos", [3], "mos3", "p")
        .model_master("pmos", [8, 49], "bsim3v3", "p")
        .model_master("pmos", [14, 54], "bsim4", "p")
        .model_master("npn", [1], "bjt", "npn")
        .model_master("pnp", [1], "bjt", "pnp")
        .model_master("d", [1], "diode", None)
        .spice_model_type("nmos")
        .spice_model_type("pmos")
        .spice_model_type("npn")
        .spice_model_type("pnp")
        .spice_model_type("d")
        .spice_model_type("njf")
        .spice_model_type("pjf")
        .spice_model_type("nmf")
        .spice_model_type("pmf")
        .spice_model_type("r")
        .spice_model_type("c")
        .spice_model_type("l")
        .spice_model_type("sw")
        .spice_model_type("csw")
        .spice_model_type("vdmos")
        .spice_model_type("ltra")
        .spice_model_type("txl")
        .spice_model_type("cpl")
        .spice_model_type("urc")
        # The circuit temperature in an expression ([CMP] 5.0 p.884).
        .temp_name("temp")
        # The VACASK analyses it has (Spectre Reference 5.0, Analysis Statements: xf, stb, pss p.771, pac p.748, pnoise p.759; no hb, dcinc or trannoise).
        .vacask_analysis("acxf")
        .vacask_analysis("acstb")
        .vacask_analysis("pss")
        .vacask_analysis("pac")
        .vacask_analysis("pnoise")
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .refuse(Construct.XyceSens, Code.W0015)
        .refuse(Construct.HspiceAnalysis, Code.W0015)
        .refuse(Construct.HspiceWLine, Code.W0001)
        .omit(Construct.VacaskScript, Code.W0120)
        .refuse(Construct.VacaskElaborate, Code.W0122)
        .refuse(Construct.CaretReread, Code.W0002)
        .lift(Construct.StatisticalCall, Rule.RackVars)
        .native(Construct.VerilogALoad)
        .refuse(Construct.OsdiLoad, Code.W0004)
        .native(Construct.OsdiInstance)
        .native(Construct.LibWholeFile)
        .omit(Construct.NgBehavior, Code.W0030)
        .native(Construct.LibSection)
        .refuse(Construct.RclModel, Code.W0006)
        .spell(Construct.ChargeCapacitor, Alt.BsourceCharge)
        .spell(Construct.BehaviouralSource, Alt.Bsource)
        .refuse(Construct.Laplace, Code.W0011)
        .native(Construct.RfPort)
        .spell(Construct.PwlRepeatV, Alt.PwlPeriod)
        .spell(Construct.PwlRepeatI, Alt.PwlPeriod)
        .refuse(Construct.NoiseWave, Code.W0009)
        .native(Construct.Conditional)
        .native(Construct.BranchParam)
        .native(Construct.Func)
        .refuse(Construct.LocalFunc, Code.W0106)
        .refuse(Construct.TableSource, Code.W0102)
        .refuse(Construct.LossyLine, Code.W0105)
        .refuse(Construct.AmWave, Code.W0108)
        .native(Construct.Step)
        .native(Construct.StepList)
        .refuse(Construct.TranStartup, Code.W0037)
        .native(Construct.DcLogList)
        .native(Construct.Tf)
        .native(Construct.Sens)
        .native(Construct.SensAc)
        .native(Construct.PoleZero)
        .native(Construct.SParameter)
        .native(Construct.Print)
        .native(Construct.Save)
        .native(Construct.UnmappedOption)
        .native(Construct.Statistics)
        .spell(Construct.Measure, Alt.SpiceRegion))


VACASK_SCALES = [
    ("meg", Scale.Exp(6)),
    ("M", Scale.Exp(6)),
    ("mil", Scale.Mil),
    ("x", Scale.Exp(6)),
    ("X", Scale.Exp(6)),
    ("m", Scale.Exp(-3)),
    ("u", Scale.Exp(-6)),
    ("n", Scale.Exp(-9)),
    ("p", Scale.Exp(-12)),
    ("f", Scale.Exp(-15)),
    ("a", Scale.Exp(-18)),
    ("k", Scale.Exp(3)),
    ("K", Scale.Exp(3)),
    ("G", Scale.Exp(9)),
    ("T", Scale.Exp(12)),
]

VACASK_KEYWORDS = [
    ("include", Tag.KwInclude),
    ("load", Tag.KwLoad),
    ("embed", Tag.KwEmbed),
    ("global", Tag.KwGlobal),
    ("ground", Tag.KwGround),
    ("parameters", Tag.KwParam),
    ("model", Tag.KwModel),
    ("subckt", Tag.KwSubckt),
    ("ends", Tag.KwEnds),
    ("section", Tag.KwSection),
    ("endsection", Tag.KwEndsection),
    ("control", Tag.KwControl),
    ("endc", Tag.KwEndc),
    ("analysis", Tag.KwAnalysis),
    ("options", Tag.KwOptions),
    ("sweep", Tag.KwSweep),
    ("save", Tag.KwSave),
    ("@if", Tag.KwIf),
    ("@elseif", Tag.KwElseif),
    ("@else", Tag.KwElse),
    ("@end", Tag.KwEndif),
]

VACASK_ANALYSES = [
    ("op", Analysis.Op),
    ("dcinc", Analysis.Dcinc),
    ("dcxf", Analysis.Dcxf),
    ("ac", Analysis.Ac),
    ("acxf", Analysis.Acxf),
    ("noise", Analysis.Noise),
    ("tran", Analysis.Tran),
    ("hb", Analysis.Hb),
]

def vacask(d):
    d = d.title(True)
    d = d.comment_tail("//")
    d = d.block_comment(True)
    d = d.long_strings(True)
    d = d.dollar(Dollar.Text)
    d = d.backslash(Backslash.Tight)
    d = d.end(None)
    d = d.stop(None)
    d = d.join(Join.Brackets)
    d = d.quote(Quote.Name)
    d = d.string(Quoting(escapes=True, protects=True, across_breaks=False))
    d = d.ident_start("$")
    d = d.ident_continue("$")
    d = d.directive(ord("@"))
    d = d.head_words(True)
    d = d.operator("**", Tag.StarStar)
    d = d.operator("==", Tag.EqualEqual)
    d = d.operator("!=", Tag.BangEqual)
    d = d.operator("<=", Tag.LessEqual)
    d = d.operator(">=", Tag.GreaterEqual)
    d = d.operator("<<", Tag.LessLess)
    d = d.operator(">>", Tag.GreaterGreater)
    d = d.operator("&&", Tag.AmpAmp)
    d = d.operator("||", Tag.PipePipe)
    d = d.caret(Caret.Xor)
    # P216-P230, docs/expr-operators.md.
    d = d.infix(Tag.PipePipe, 2)
    d = d.infix(Tag.AmpAmp, 3)
    d = d.infix(Tag.Pipe, 4)
    d = d.infix(Tag.Caret, 5)
    d = d.infix(Tag.Amp, 6)
    d = d.infix(Tag.EqualEqual, 7)
    d = d.infix(Tag.BangEqual, 7)
    d = d.infix(Tag.LessEqual, 8)
    d = d.infix(Tag.GreaterEqual, 8)
    d = d.infix(Tag.Less, 8)
    d = d.infix(Tag.Greater, 8)
    d = d.infix(Tag.LessLess, 9)
    d = d.infix(Tag.GreaterGreater, 9)
    d = d.infix(Tag.Plus, 10)
    d = d.infix(Tag.Minus, 10)
    d = d.infix(Tag.Star, 11)
    d = d.infix(Tag.Slash, 11)
    d = d.infix(Tag.StarStar, 12)
    d = d.right(Tag.StarStar)
    d = d.prefix(Tag.Plus)
    d = d.prefix(Tag.Minus)
    d = d.prefix(Tag.Bang)
    d = d.prefix(Tag.Tilde)
    d = d.prefix_bp(13)
    d = d.index(True)
    d = d.lib_sections(True)
    d = d.exponent("eE")
    for s, k in VACASK_SCALES:
        d = d.scale(s, k)
    d = d.tail(Tail.Letters(after_exponent=False, hex=True))
    d = d.arithmetic(Arithmetic.Decimal)
    for s, t in VACASK_KEYWORDS:
        d = d.keyword(s, t)
    d = d.named(True)
    # An instance's master is a model or a subcircuit, never a device word
    # the modules a `model` names are `device::VACASK`, which resolve reads.
    for s, a in VACASK_ANALYSES:
        d = d.analysis(s, a)
    return (d
        # The spelling.
        .comment("")
        .path_quote("\"")
        .expr("", "")
        .func(FuncForm.None_)
        .subckt_params(None)
        # The behaviours.
        .writer(Writer.Vacask)
        .caret_power(False)
        .named_rules(NamedRules.Vacask)
        .options_form(OptionsForm.Vacask)
        .named_params(NamedParams.Vacask)
        .analysis_masters(Lang.SPECTRE)
        # `.options KEY=VALUE` whose VACASK spelling is the same word and
        # the same quantity (`itl1`, `maxord` are rejected there). `scale`
        # is the `$simparam("scale")` VACASK's `sp_*` modules read where
        # ngspice's code reads `CKTscale` (`devices/spice/*.va` headers;
        # `lib/osdidevice.cpp` `simParamNames`).
        .option_to("reltol", "reltol")
        .option_to("abstol", "abstol")
        .option_to("vntol", "vntol")
        .option_to("chgtol", "chgtol")
        .option_to("gmin", "gmin")
        .option_to("temp", "temp")
        .option_to("tnom", "tnom")
        .option_to("scale", "scale")
        # `method=trap|gear` is `tran_method` (`docs/cmd-options-tran.md`).
        .option_method("method")
        # Keys that only steer a simulator's bookkeeping or printing:
        # dropping one changes no number in the raw file.
        .option_cosmetic("noacct")
        .option_cosmetic("acct")
        .option_cosmetic("list")
        .option_cosmetic("node")
        .option_cosmetic("post")
        .option_cosmetic("trans")
        .option_cosmetic("nopage")
        .option_cosmetic("nomod")
        .option_cosmetic("lvlcod")
        .option_cosmetic("klu")
        .option_cosmetic("sparse")
        .option_cosmetic("savecurrents")
        .option_cosmetic("numdgt")
        # The module a SPICE `.model` type gets at a LEVEL (VACASK 36be560
        # `devices/spice/*.va`: `sp_<module>`, `type=±1`; ngspice-45
        # `inpdomod.c` LEVEL per type): `.model x NPN (LEVEL=4)` is VBIC, other
        # equations, so no row. R, C, L and switch types have none (`sp_resistor`
        # renames the card's `r`/`tc1` to `model_r`…); the line types are no
        # SPICE type here: passed through ("Model not found" beats a substitute).
        .model_master("nmos", [1], "mos1", "1")
        .model_master("nmos", [2], "mos2", "1")
        .model_master("nmos", [3], "mos3", "1")
        .model_master("nmos", [6], "mos6", "1")
        .model_master("nmos", [9], "mos9", "1")
        .model_master("nmos", [8, 49], "bsim3v3", "1")
        .model_master("nmos", [14, 54], "bsim4v8", "1")
        .model_master("pmos", [1], "mos1", "-1")
        .model_master("pmos", [2], "mos2", "-1")
        .model_master("pmos", [3], "mos3", "-1")
        .model_master("pmos", [6], "mos6", "-1")
        .model_master("pmos", [9], "mos9", "-1")
        .model_master("pmos", [8, 49], "bsim3v3", "-1")
        .model_master("pmos", [14, 54], "bsim4v8", "-1")
        .model_master("d", [1], "diode", None)
        .model_master("npn", [1], "bjt", "1")
        .model_master("pnp", [1], "bjt", "-1")
        .model_master("njf", [1], "jfet1", "1")
        .model_master("pjf", [1], "jfet1", "-1")
        .model_master("nmf", [1], "mes1", "1")
        .model_master("pmf", [1], "mes1", "-1")
        .model_master("vdmos", [1], "vdmos", "1")
        .spice_model_type("nmos")
        .spice_model_type("pmos")
        .spice_model_type("npn")
        .spice_model_type("pnp")
        .spice_model_type("d")
        .spice_model_type("njf")
        .spice_model_type("pjf")
        .spice_model_type("nmf")
        .spice_model_type("pmf")
        .spice_model_type("vdmos")
        .spice_model_type("r")
        .spice_model_type("c")
        .spice_model_type("l")
        .spice_model_type("sw")
        .spice_model_type("csw")
        .spice_model_type("res")
        .spice_model_type("cap")
        .spice_model_type("ind")
        .spectre_analysis("hb")
        .spectre_analysis("sweep")
        .spectre_analysis("xf")
        .spectre_analysis("montecarlo")
        # The circuit temperature in an expression (docs/expr-special.md).
        .temp_name("$temp")
        # docs/expr-constants.md: the builtin constants and their values.
        .builtin_constant("M_E", "2.718281828459045")
        .builtin_constant("M_LOG2E", "1.4426950408889634")
        .builtin_constant("M_LOG10E", "0.4342944819032518")
        .builtin_constant("M_LN2", "0.6931471805599453")
        .builtin_constant("M_LN10", "2.302585092994046")
        .builtin_constant("M_PI", "3.141592653589793")
        .builtin_constant("M_TWO_PI", "6.283185307179586")
        .builtin_constant("M_PI_2", "1.5707963267948966")
        .builtin_constant("M_PI_4", "0.7853981633974483")
        .builtin_constant("M_1_PI", "0.3183098861837907")
        .builtin_constant("M_2_PI", "0.6366197723675814")
        .builtin_constant("M_2_SQRTPI", "1.1283791670955126")
        .builtin_constant("M_SQRT2", "1.4142135623730951")
        .builtin_constant("M_SQRT1_2", "0.7071067811865475")
        .builtin_constant("M_DEGPERRAD", "57.29577951308232")
        .builtin_constant("P_Q", "1.6021918e-19")
        .builtin_constant("P_C", "299792456.2")
        .builtin_constant("P_K", "1.3806226e-23")
        .builtin_constant("P_H", "6.6260755e-34")
        .builtin_constant("P_EPS0", "8.854187924e-12")
        .builtin_constant("P_U0", "1.2566370614359173e-06")
        .builtin_constant("P_CELSIUS0", "273.15")
        # The VACASK analyses it has (docs/cmd-analysis-*.md).
        .vacask_analysis("dcinc")
        .vacask_analysis("acxf")
        .vacask_analysis("acstb")
        .vacask_analysis("hb")
        .vacask_analysis("hbac")
        .vacask_analysis("hbnoise")
        .vacask_analysis("pss")
        .vacask_analysis("pac")
        .vacask_analysis("pnoise")
        .vacask_analysis("trannoise")
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .refuse(Construct.XyceSens, Code.W0015)
        .refuse(Construct.HspiceAnalysis, Code.W0015)
        .refuse(Construct.HspiceWLine, Code.W0105)
        .native(Construct.VacaskScript)
        .native(Construct.VacaskElaborate)
        .refuse(Construct.CaretReread, Code.W0002)
        .lift(Construct.StatisticalCall, Rule.VacaskCalls)
        .native(Construct.VerilogALoad)
        .native(Construct.OsdiLoad)
        .native(Construct.OsdiInstance)
        .native(Construct.LibWholeFile)
        .omit(Construct.NgBehavior, Code.W0030)
        .native(Construct.LibSection)
        .refuse(Construct.RclModel, Code.W0006)
        .refuse(Construct.ChargeCapacitor, Code.W0105)
        .refuse(Construct.BehaviouralSource, Code.W0102)
        .refuse(Construct.Laplace, Code.W0011)
        .refuse(Construct.RfPort, Code.W0007)
        .refuse(Construct.PwlRepeatV, Code.W0008)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .refuse(Construct.NoiseWave, Code.W0009)
        .native(Construct.Conditional)
        .refuse(Construct.BranchParam, Code.W0014)
        .refuse(Construct.Func, Code.W0106)
        .refuse(Construct.LocalFunc, Code.W0106)
        .refuse(Construct.TableSource, Code.W0102)
        .refuse(Construct.LossyLine, Code.W0105)
        .native(Construct.AmWave)
        .native(Construct.Step)
        .native(Construct.StepList)
        .refuse(Construct.TranStartup, Code.W0037)
        .native(Construct.DcLogList)
        .native(Construct.Tf)
        .refuse(Construct.Sens, Code.W0015)
        .refuse(Construct.SensAc, Code.W0015)
        .refuse(Construct.PoleZero, Code.W0015)
        .refuse(Construct.SParameter, Code.W0015)
        .native(Construct.Print)
        .omit(Construct.Save, Code.W0023)
        .refuse(Construct.UnmappedOption, Code.W0017)
        .lift(Construct.Statistics, Rule.StatisticsDraws)
        .refuse(Construct.Measure, Code.W0109))


def cdl(d):
    return (d
        .parasitics(Parasitics.Netlist)
        .writer(Writer.Cdl)
        # The spelling: ngspice's cards.
        .write_lexicon(Lang.NGSPICE)
        .comment("*")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm.Func)
        .subckt_params("PARAMS:")
        .ngspice_numbers(True)
        # The capability column.
        .omit(Construct.ForeignText, Code.W0028)
        .copy(Construct.PrivateText, Code.W0027)
        .copy(Construct.XyceSens, Code.W0027)
        .copy(Construct.HspiceAnalysis, Code.W0027)
        .copy(Construct.HspiceWLine, Code.W0027)
        .copy(Construct.VacaskScript, Code.W0027)
        .copy(Construct.VacaskElaborate, Code.W0027)
        .refuse(Construct.CaretReread, Code.W0002)
        .native(Construct.StatisticalCall)
        .native(Construct.VerilogALoad)
        .native(Construct.OsdiLoad)
        .native(Construct.OsdiInstance)
        .refuse(Construct.LibWholeFile, Code.W0005)
        .omit(Construct.NgBehavior, Code.W0030)
        .native(Construct.LibSection)
        .native(Construct.RclModel)
        .native(Construct.ChargeCapacitor)
        .native(Construct.BehaviouralSource)
        .lift(Construct.Laplace, Rule.LaplaceXfer)
        .native(Construct.RfPort)
        .native(Construct.PwlRepeatV)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .native(Construct.NoiseWave)
        .native(Construct.Conditional)
        .native(Construct.BranchParam)
        .native(Construct.Func)
        .native(Construct.LocalFunc)
        .native(Construct.TableSource)
        .native(Construct.LossyLine)
        .native(Construct.AmWave)
        .native(Construct.Step)
        .native(Construct.StepList)
        .native(Construct.TranStartup)
        .native(Construct.DcLogList)
        .native(Construct.Tf)
        .native(Construct.Sens)
        .native(Construct.SensAc)
        .native(Construct.PoleZero)
        .native(Construct.SParameter)
        .native(Construct.Print)
        .native(Construct.Save)
        .native(Construct.UnmappedOption)
        .native(Construct.Statistics)
        .native(Construct.Measure))


def verilog(d):
    return (d
        .parasitics(Parasitics.Netlist)
        .writer(Writer.Verilog)
        # The spelling: ngspice's cards.
        .write_lexicon(Lang.NGSPICE)
        .comment("//")
        .path_quote("")
        .expr("{", "}")
        .func(FuncForm.Func)
        .subckt_params("PARAMS:")
        .ngspice_numbers(True)
        # The capability column.
        .refuse(Construct.ForeignText, Code.W0001)
        .refuse(Construct.PrivateText, Code.W0039)
        .refuse(Construct.XyceSens, Code.W0001)
        .refuse(Construct.HspiceAnalysis, Code.W0001)
        .refuse(Construct.HspiceWLine, Code.W0001)
        .omit(Construct.VacaskScript, Code.W0120)
        .refuse(Construct.VacaskElaborate, Code.W0122)
        .refuse(Construct.CaretReread, Code.W0002)
        .native(Construct.StatisticalCall)
        .native(Construct.VerilogALoad)
        .native(Construct.OsdiLoad)
        .omit(Construct.OsdiInstance, Code.W0029)
        .refuse(Construct.LibWholeFile, Code.W0005)
        .omit(Construct.NgBehavior, Code.W0030)
        .native(Construct.LibSection)
        .native(Construct.RclModel)
        .native(Construct.ChargeCapacitor)
        .native(Construct.BehaviouralSource)
        .refuse(Construct.Laplace, Code.W0011)
        .native(Construct.RfPort)
        .refuse(Construct.PwlRepeatV, Code.W0008)
        .refuse(Construct.PwlRepeatI, Code.W0008)
        .refuse(Construct.NoiseWave, Code.W0009)
        .native(Construct.Conditional)
        .native(Construct.BranchParam)
        .native(Construct.Func)
        .native(Construct.LocalFunc)
        .native(Construct.TableSource)
        .native(Construct.LossyLine)
        .native(Construct.AmWave)
        .native(Construct.Step)
        .native(Construct.StepList)
        .native(Construct.TranStartup)
        .native(Construct.DcLogList)
        .native(Construct.Tf)
        .native(Construct.Sens)
        .native(Construct.SensAc)
        .native(Construct.PoleZero)
        .native(Construct.SParameter)
        .native(Construct.Print)
        .native(Construct.Save)
        .native(Construct.UnmappedOption)
        .native(Construct.Statistics)
        .native(Construct.Measure))


def ngspice_given(d):
    return (d
        .extends(Lang.NGSPICE)
        .text_family(Lang.NGSPICE)
        .arithmetic(Arithmetic.Decimal)
        .keyword(".step", Tag.KwStep)
        .region("ngspice", Lang.NGSPICE)
        .region("spectre", Lang.SPECTRE)
        .region("vacask", Lang.VACASK)
        .region("hspice", Lang.HSPICE)
        .region("ltspice", Lang.LTSPICE)
        .region("xyce", Lang.XYCE)
        .regions_registered(True))


# Each built-in: its function here and its registered Rust row.
BUILTINS = {
    "NGSPICE": (ngspice, Lang.NGSPICE),
    "ESPICE": (espice, Lang.ESPICE),
    "HSPICE": (hspice, Lang.HSPICE),
    "XYCE": (xyce, Lang.XYCE),
    "LTSPICE": (ltspice, Lang.LTSPICE),
    "SPECTRE": (spectre, Lang.SPECTRE),
    "VACASK": (vacask, Lang.VACASK),
    "CDL": (cdl, Lang.CDL),
    "VERILOG": (verilog, Lang.VERILOG),
    "NGSPICE_GIVEN": (ngspice_given, Lang.NGSPICE_GIVEN),
}
