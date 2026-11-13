"""conformance.py: every HSPICE deck of the VAMS conformance suite ($HSPICE_CORPUS, the flake's
OpenFAT/VAMS-Conformance-Suite tests/netlist; decks whose path holds `hspice`) run on ESPice in
HSPICE mode, against the deck's own `<deck>.expected.json`: the status (success or error) and every
plot column value it lists, within its rtol/atol. `checks` and measure expectations are reported as
unchecked. Prints one line per deck and a summary. Run with `python3 -I` inside `nix develop .#testing`.
"""
import json, math, os, shutil, subprocess, sys, tempfile

ROOT = os.environ["HSPICE_CORPUS"]

def plots(path):
    """Plots of an ascii raw file: [(name, {var: [values]})]."""
    out, lines, i = [], open(path, errors="replace").read().split("\n"), 0
    while i < len(lines):
        if not lines[i].startswith("Plotname:"):
            i += 1
            continue
        name, nv, npts, names = lines[i].split(":", 1)[1].strip(), 0, 0, []
        while not lines[i].startswith("Values:"):
            if lines[i].startswith("No. Variables:"): nv = int(lines[i].split(":")[1])
            if lines[i].startswith("No. Points:"): npts = int(lines[i].split(":")[1])
            if lines[i].startswith("Variables:"):
                names = [lines[i + 1 + k].split()[1].lower() for k in range(nv)]
            i += 1
        toks, i = [], i + 1
        while len(toks) < npts * (nv + 1) and i < len(lines):
            toks += lines[i].split(); i += 1
        cols = {n: [] for n in names}
        for r in range(npts):
            row = toks[r * (nv + 1) + 1:(r + 1) * (nv + 1)]
            for n, v in zip(names, row):
                cols[n].append(complex(*map(float, v.split(","))) if "," in v else float(v))
        out.append((name, cols))
    return out

def interp(xs, ys, x):
    """`ys` at `x` by linear interpolation in `xs` (sorted); the last row at the end; None outside."""
    for i in range(1, len(xs)):
        if xs[i - 1] <= x <= xs[i]:
            f = 0 if xs[i] == xs[i - 1] else (x - xs[i - 1]) / (xs[i] - xs[i - 1])
            return ys[i - 1] + f * (ys[i] - ys[i - 1])
    if xs and abs(x - xs[-1]) <= 1e-9 * abs(x):
        return ys[-1]
    return ys[0] if xs and x == xs[0] else None

def check(deck, exp):
    d = os.path.dirname(deck)
    with tempfile.TemporaryDirectory() as tmp:
        w = os.path.join(tmp, "d"); shutil.copytree(d, w)
        for dp, ds, fs in os.walk(w):  # the store copy is read-only
            for x in ds + fs + [""]: os.chmod(os.path.join(dp, x), 0o755)
        p = subprocess.run(["espice-hspice", "--tokenizer=hspice", "--format=ascii", "-r", "out.raw",
                            os.path.basename(deck)], cwd=w, capture_output=True, text=True, timeout=600)
        want = exp["expect"]["status"]
        got = "success" if p.returncode == 0 else "error"
        if got != want:
            err = [l for l in (p.stdout + p.stderr).split("\n") if "rror" in l][:1]
            return f"FAIL status {got}, expected {want} {err}"
        if got == "error":
            return "ok (error as expected)"
        have = plots(os.path.join(w, "out.raw")) if os.path.exists(os.path.join(w, "out.raw")) else []
        notes = []
        for pl in exp["expect"].get("plots", []):
            match = [c for n, c in have if n.lower().startswith(pl["name"].lower())]
            if not match:
                return f"FAIL no plot {pl['name']!r} (have {[n for n, _ in have]})"
            cols = match[0]
            axis = pl.get("axis")
            xs = cols.get(axis["name"].lower()) if axis else None
            if axis and xs is None:
                return f"FAIL plot {pl['name']!r} has no axis {axis['name']}"
            for col, spec in pl["columns"].items():
                if col.lower() not in cols:
                    return f"FAIL plot {pl['name']!r} has no column {col}"
                got_v = cols[col.lower()]
                if axis:
                    at = axis["values"]
                else:
                    at = list(pl.get("row_indices") or range(len(spec["values"])))
                for row, (k, v) in enumerate(zip(at, spec["values"])):
                    if isinstance(v, list): v = complex(v[0], v[1])
                    if axis and pl["match"] == "samples":
                        g = interp([x.real if isinstance(x, complex) else x for x in xs], got_v, k)
                    elif axis:  # "exact": the listed rows in order, each on its axis value
                        x = xs[row] if row < len(xs) else None
                        x = x.real if isinstance(x, complex) else x
                        if x is None or abs(x - k) > axis.get("atol", 0) + axis.get("rtol", 0) * abs(k):
                            return f"FAIL row {row}: {axis['name']} {x}, expected {k}"
                        g = got_v[row]
                    else:
                        if k >= len(got_v):
                            return f"FAIL {col}[{k}] missing ({len(got_v)} rows)"
                        g = got_v[k]
                    if g is None or abs(g - v) > spec.get("atol", 0) + spec.get("rtol", 0) * abs(v):
                        return f"FAIL {col} at {k} = {g}, expected {v}"
        for k in ("checks",):
            if k in exp["expect"]: notes.append(f"{k} unchecked")
        return "ok" + (f" ({', '.join(notes)})" if notes else "")

decks = sorted(os.path.join(dp, f) for dp, _, fs in os.walk(ROOT) for f in fs
               if f.endswith(".sp") and "hspice" in os.path.join(dp, f)[len(ROOT):])
n = {"ok": 0, "FAIL": 0}
for deck in decks:
    exp = json.load(open(deck[:-3] + ".expected.json"))
    try:
        r = check(deck, exp)
    except Exception as e:
        r = f"FAIL harness {type(e).__name__}: {e}"
    n["ok" if r.startswith("ok") else "FAIL"] += 1
    print(f"{os.path.relpath(deck, ROOT)}: {r}")
print(f"summary: {n['ok']} ok, {n['FAIL']} FAIL of {len(decks)}")
