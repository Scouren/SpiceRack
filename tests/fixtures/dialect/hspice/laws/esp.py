"""esp.py DIR: every .cir of DIR (an ngspice law deck: cards, then a .control block with one
analysis and one print) rewritten as HSPICE text (the analysis as its dot card, no control block)
and run on ESPice in HSPICE mode (espice-hspice --tokenizer=hspice); prints deck and the printed
quantity's last value. ESPice is not HSPICE."""
import cmath, os, re, subprocess, sys, tempfile
d = sys.argv[1]
for f in sorted(os.listdir(d)):
    if not f.endswith(".cir"): continue
    lines = open(os.path.join(d, f)).read().split("\n")
    out, ctl, an, pr = [], False, None, None
    for l in lines:
        low = l.strip().lower()
        if low == ".control": ctl = True; continue
        if low == ".endc": ctl = False; continue
        if ctl:
            w = low.split()
            if not w or w[0] == "set": continue
            if w[0] == "print": pr = " ".join(w[1:]); continue
            an = "." + l.strip(); continue
        if low == ".end": continue
        out.append(l)
    # the ngspice decks' temperatures: 27 C and tnom 27 unless the deck sets them (HSPICE's
    # default is 25 C, R102)
    low_all = "\n".join(out).lower()
    if os.environ.get("KEEP_TEMP") is None:
        if ".temp" not in low_all: out.append(".temp 27")
        if "tnom" not in low_all: out.append(".option tnom=27")
    out += [an, ".end", ""]
    with tempfile.TemporaryDirectory() as t:
        open(os.path.join(t, "d.sp"), "w").write("\n".join(out))
        p = subprocess.run(["espice-hspice", "--tokenizer=hspice", "--format=print", "-r", "o.txt", "d.sp"],
                           cwd=t, capture_output=True, text=True, timeout=300)
        if p.returncode != 0:
            err = [x for x in (p.stdout + p.stderr).split("\n") if "rror" in x][:1]
            print(f"{f[:-4]:22} refused {err}"); continue
        rows = [r.split("\t") for r in open(os.path.join(t, "o.txt")).read().split("\n") if "\t" in r]
        head = rows[0]; last = rows[-1]
        def col(name):
            if name not in head: return None
            k = head.index(name); re_ = float(last[k])
            im = float(last[k + 1]) if k + 1 < len(head) and head[k + 1] == "(imag)" else 0.0
            return complex(re_, im)
        m = re.match(r"(mag\()?(.*?)\)?$", pr)
        e = pr[4:-1] if pr.startswith("mag(") else pr
        mv = re.match(r"v\(([^,]+),([^)]+)\)", e)
        val = (col(f"v({mv.group(1)})") - col(f"v({mv.group(2)})")) if mv else col(e)
        if val is None: print(f"{f[:-4]:22} no column {e} in {head[:8]}"); continue
        v = abs(val) if pr.startswith("mag(") else val.real
        print(f"{f[:-4]:22} espice={v:.12e}")
