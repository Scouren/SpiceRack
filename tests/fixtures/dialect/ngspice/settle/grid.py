#!/usr/bin/env python3
"""The function-grid decks (R76): every function and operator ngspice's reader maps to the IR
(src/dialect/ngspice/tables.rs FUNCS, expr_lex.rs operators), on a grid inside its domain.

grid_numparam.sp: one `V` source per point, its value `{f(args)}` (numparam).
grid_beh_<f>.sp: per function, one `B` source per point, `V = f(v(a..))`, the arguments node voltages.
grid_fold.sp: the same `B` sources with literal arguments.

Run from this directory: `python3 grid.py`; tests/dialect_ngspice_grid.rs runs ngspice on them.
"""

ALL = [-3, -2.5, -1.5, -1, -0.5, -1e-07, 0, 1e-07, 0.5, 1, 1.5, 2.5, 3, -1000.0, 1000.0]
HYP = [x for x in ALL if abs(x) < 100] + [-30, 30]
EXP = HYP + [-50, 50, -250, 250]
POS0 = [x for x in ALL if x >= 0]
POS = [x for x in ALL if x > 0]
UNIT = [-1, -0.99, -0.5, -1e-07, 0, 1e-07, 0.5, 0.99, 1]
OPEN = [-0.99, -0.5, -1e-07, 0, 1e-07, 0.5, 0.99]
ACOSH = [1, 1.5, 2.5, 3, 1000.0]
X2 = [-2.5, -1, -0.5, 0, 0.5, 1, 2.5]
Y2 = [-2, -1, -0.5, 0, 0.5, 1, 2, 3]
COND = [-1, 0, 0.5, 1]
# B points where the slope is infinite: ngspice solves v = f(x0) + f'(x0)(x - x0) with f' from
# PTdivide's 1/(0 + 1e-32), and the companion cancels f (asin(1) prints 0); folded grid only
STEEP = {("asin", 1), ("asin", -1), ("acos", 1), ("acos", -1), ("acosh", 1), ("sqrt", 0)}
# B points outside every evaluator's agreement that still give a value (ptfuncs.c: MODULUS past
# 2^31, exp above 227.9559242, ln at 0); folded grid only, numparam errors on each
EDGE = [("sin({0})", (x,)) for x in (1e10, 2e10, -2e10)] + [("cos({0})", (1e11,)),
        ("tan({0})", (1e10,)), ("exp({0})", (1000.0,)), ("ln({0})", (0,)), ("log({0})", (0,)),
        ("log10({0})", (0,))]


def pairs(xs, ys, keep):
    return [(x, y) for x in xs for y in ys if keep(x, y)]


def no_zero_neg(x, y):
    return not (x == 0 and y < 0)


def c_pow(x, y):
    return no_zero_neg(x, y) and (x >= 0 or float(y).is_integer())


ONE_ARG = {
    "abs": ALL, "sgn": ALL, "floor": ALL, "ceil": ALL, "int": ALL, "nint": ALL + [-2.5, 2.5, 0.5],
    "sqr": ALL, "sin": ALL, "cos": ALL, "tan": ALL, "atan": ALL, "arctan": ALL, "tanh": ALL,
    "asinh": ALL, "sinh": HYP, "cosh": HYP, "exp": EXP, "sqrt": POS0, "ln": POS, "log": POS,
    "log10": POS, "asin": UNIT, "acos": UNIT, "atanh": OPEN, "acosh": ACOSH,
    "u": ALL, "uramp": ALL, "u2": ALL + [0.25, 0.75],
}
TWO_ARG = {
    "pow": None, "pwr": None, "min": pairs(X2, X2, lambda x, y: True),
    "max": pairs(X2, X2, lambda x, y: True),
}
# the names each evaluator maps (tables.rs FUNCS)
NUMPARAM = ["abs", "sqrt", "exp", "ln", "log", "log10", "sin", "cos", "tan", "asin", "acos",
            "atan", "arctan", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "pow", "pwr",
            "min", "max", "int", "nint", "sgn", "ceil", "floor", "sqr"]
BEHAVIOURAL = ["abs", "sqrt", "exp", "ln", "log", "log10", "sin", "cos", "tan", "asin", "acos",
               "atan", "sinh", "cosh", "tanh", "asinh", "acosh", "atanh", "pow", "pwr", "min",
               "max", "nint", "sgn", "ceil", "floor", "uramp", "u", "u2"]


def num(x):
    return repr(float(x)) if not float(x).is_integer() or abs(x) >= 1000 else str(int(x))


def points(numparam):
    """(text template, args) per point; `{0}`, `{1}` stand for the arguments."""
    out = []
    for f in NUMPARAM if numparam else BEHAVIOURAL:
        if f in ONE_ARG:
            out += [(f + "({0})", (x,)) for x in ONE_ARG[f]]
            continue
        if f in ("min", "max"):
            grid = TWO_ARG[f]
        elif f == "pow" and numparam:
            grid = pairs(X2, Y2, c_pow)  # numparam pow is C's pow
        else:
            grid = pairs(X2, Y2, no_zero_neg)  # pow(|x|, y), or pwr's sign(x)|x|^y
        out += [(f + "({0}, {1})", a) for a in grid]
    for op in ("**", "^"):
        out += [("({0})" + op + "({1})", a) for a in pairs(X2, Y2, no_zero_neg)]
    if numparam:
        nz = pairs(X2, X2, lambda x, y: y != 0)
        out += [("({0}) % ({1})", a) for a in nz]
        out += [("({0}) \\ ({1})", a) for a in nz]
    for c in COND:
        out += [("({0}) ? (2) : (3)", (c,)), ("ternary_fcn({0}, 2, 3)", (c,))]
    return out


def name(template):
    """The function or operator a point's template applies (`pow`, `**`, `?`, ...)."""
    if template.startswith("("):
        return {"*": "pow_op", "^": "caret", "?": "tern"}[template[5:].lstrip(") ")[0]]
    return template.split("(")[0]


def write(name, title, lines):
    with open(name, "w") as f:
        f.write(f"{title}\n* settle grid: values in the raw file (ngspice -b -r), compared with the "
                "IR's reference evaluator by tests/dialect_ngspice_grid.rs\n")
        f.write("\n".join(lines) + "\n.op\n.end\n")


def main():
    np = points(True)
    write("grid_numparam.sp", "grid_numparam",
          [f"vg{k} g{k} 0 {{{t.format(*map(num, a))}}}" for k, (t, a) in enumerate(np)])
    beh = points(False)
    for f in sorted({name(t) for t, _ in beh}):
        mine = [(k, t, a) for k, (t, a) in enumerate(beh)
                if name(t) == f and (f, a[0]) not in STEEP]
        values = sorted({x for _, _, a in mine for x in a})
        node = {x: f"a{i}" for i, x in enumerate(values)}
        args = [f"va{i} a{i} 0 dc {num(x)}" for i, x in enumerate(values)]
        sets = [".nodeset " + " ".join(f"v(a{i})={num(x)}" for i, x in enumerate(values))]
        write(f"grid_beh_{f}.sp", f"grid_beh_{f}", args + sets + [
            f"bd{k} d{k} 0 v = {t.format(*(f'v({node[x]})' for x in a))}" for k, t, a in mine])
    write("grid_fold.sp", "grid_fold",
          [f"bf{k} f{k} 0 v = {t.format(*map(num, a))}" for k, (t, a) in enumerate(beh + EDGE)])


main()
