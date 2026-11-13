#!/usr/bin/env python3
import math
"""Generates grid_static.sim and grid_dynamic.sim: every function VACASK's reader maps (R76's
function-grid law). Run from this directory: ./grid.py. The decks are committed; the test
(tests/dialect_vacask/grid.rs) runs them on VACASK and compares each value with the IR's
reference evaluator on what the reader makes of the same text."""

UNARY = ["sin", "cos", "tan", "asin", "acos", "atan", "sinh", "cosh", "tanh", "asinh", "acosh",
         "atanh", "exp", "log", "ln", "log10", "sqrt", "abs", "floor", "ceil", "round", "integer",
         "int", "sgn"]
BINARY = ["pow", "hypot", "fmod", "min", "max", "atan2", "sign"]
STATIC_ONLY = {"round", "integer", "fmod"}  # tables::DYNAMIC_LACKS
# VACASK rejects these in a behavioural source ("Function sign() with arity 2 cannot be
# translated to Verilog-A."): a reader finding, checked apart (grid.rs).
NOT_DYNAMIC = {"sign", "int"}  # int: "'$rtoi' was not found" (OpenVAF-r)
REALS = ["-2.5", "-1.0", "-0.5", "-0.0", "0.0", "0.5", "1.0", "2.5", "1e-3", "1e3"]
INTS = ["-3", "0", "3"]
PAIRS = [(a, b) for a in ["-2.0", "-0.5", "0.0", "0.5", "2.0", "-3", "3", "0"]
         for b in ["-2.0", "-0.5", "-0.0", "0.0", "0.5", "2.0", "-3", "3", "0"]]

def calls():
    out = []
    for f in UNARY:
        for x in REALS + INTS:
            out.append((f, (x,)))
    for f in BINARY:
        for p in PAIRS:
            out.append((f, p))
    return out

PY = {"sin": math.sin, "cos": math.cos, "tan": math.tan, "asin": math.asin, "acos": math.acos,
      "atan": math.atan, "sinh": math.sinh, "cosh": math.cosh, "tanh": math.tanh,
      "asinh": math.asinh, "acosh": math.acosh, "atanh": math.atanh, "exp": math.exp,
      "log": math.log, "ln": math.log, "log10": math.log10, "sqrt": math.sqrt, "abs": abs,
      "floor": math.floor, "ceil": math.ceil, "int": int, "sgn": lambda x: 1.0,
      "pow": math.pow, "hypot": math.hypot, "min": min, "max": max,
      "atan2": math.atan2, "sign": math.copysign,
      "round": round, "integer": int, "fmod": math.fmod}

def finite(f, args):
    """Whether the point is inside the function's domain (VACASK stops on a "Domain error." in a
    parameter and an op aborts on a NaN or an inf): only the point set, never the expected
    value, comes from Python."""
    xs = [float(a) for a in args]
    # VACASK: "Zero raised to negative or zero power." (C's pow(0, 0) is 1).
    if f == "pow" and xs[0] == 0.0 and xs[1] <= 0.0:
        return False
    # VACASK's static atan2 is atan(y/x): NaN at the origin (an op with a NaN aborts).
    if f == "atan2" and xs[0] == 0.0 and xs[1] == 0.0:
        return False
    try:
        v = PY[f](*xs)
    except (ValueError, OverflowError, ZeroDivisionError):
        return False
    return math.isfinite(v)

def static():
    """One deck: each value a parameter, read by a source's `dc` (the parameter evaluator); `h`
    is the value halved, which shows an integer result's type."""
    lines = ["functions on a grid, parameter evaluator", "model vsource vsource"]
    for k, (f, args) in enumerate(calls()):
        # A parameter outside its function's domain is "Domain error." and stops the deck.
        if not finite(f, args):
            continue
        e = f"{f}({', '.join(args)})"
        lines.append(f"parameters g{k}={e} h{k}=({e})/2")
        lines.append(f"vg{k} (g{k} 0) vsource dc=g{k}")
        lines.append(f"vh{k} (h{k} 0) vsource dc=h{k}")
    lines += ["control", '  options rawfile="ascii"', "  analysis op1 op", "endc"]
    return "\n".join(lines) + "\n"

def smooth(f, args):
    """Whether Newton can solve the point when its arguments are read from a node: no infinite
    or undefined derivative there (an op whose Jacobian holds one aborts)."""
    xs = [float(a) for a in args]
    if f == "sqrt" and xs[0] == 0.0:
        return False
    if f in ("asin", "acos", "atanh") and abs(xs[0]) == 1.0:
        return False
    if f == "acosh" and xs[0] == 1.0:
        return False
    if f in ("hypot", "atan2") and xs[0] == 0.0 and xs[1] == 0.0:
        return False
    if f == "pow" and xs[0] <= 0.0:
        return False
    return True

def behavioural(f, fold):
    """One deck per function: each value a behavioural source's, driving a resistor (a lone
    source left the matrix singular). `fold`: literal arguments, which VACASK evaluates with
    its parameter evaluator before the module is compiled; else each argument read from the
    node `one` (1 V), the compiled expression."""
    kind = "folded" if fold else "behavioural"
    lines = [f"{f} on a grid, {kind} sources", 'load "resistor.osdi"', "model r resistor",
             "model vsource vsource", "vone (one 0) vsource dc=1"]
    for k, (g, args) in enumerate(calls()):
        if g != f or not finite(f, args) or (not fold and not smooth(f, args)):
            continue
        # OpenVAF-r crashes compiling `ceil(0)` or `ceil(0.0)` (exit 101); `ceil(-0.0)` runs.
        if f == "ceil" and args[0] in ("0", "0.0"):
            continue
        # `$temp/$temp` is 1 and keeps the expression the compiled module's (settle
        # atan2_forms); a node's voltage starts Newton at 0, where ln, sqrt or acosh fail.
        shown = args if fold else [f"{a}*($temp/$temp)" for a in args]
        lines.append(f"b{k} (d{k} 0) v={f}({', '.join(shown)})")
        lines.append(f"r{k} (d{k} 0) r r=1k")
    lines += ["control", '  options rawfile="ascii"', "  analysis op1 op", "endc"]
    return "\n".join(lines) + "\n"

open("grid_static.sim", "w").write(static())
for f in UNARY + BINARY:
    if f not in STATIC_ONLY and f not in NOT_DYNAMIC:
        open(f"grid_dyn_{f}.sim", "w").write(behavioural(f, False))
        open(f"grid_fold_{f}.sim", "w").write(behavioural(f, True))
