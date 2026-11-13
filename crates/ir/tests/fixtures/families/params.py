"""params.py CDIR PREFIX VA: parameter names and defaults, ngspice-45 C source against a VACASK module.
C: names from the IFparm tables (*.c: IOP/IP/OP..("name", CONST, ...)), CONST -> field from the
param/mparam switches (case CONST: ... ->FIELD = value), FIELD -> default from setup/temp
(`if (!...FIELDGiven) ... FIELD = value;`). VA: `parameter <type> name = default;` and aliasparam."""
import glob, re, sys
cdir, prefix, va = sys.argv[1:4]
csrc = {f: open(f, errors='replace').read() for f in glob.glob(cdir + '/*.c')}
allc = '\n'.join(csrc.values())
names = {}  # name -> const
for m in re.finditer(r'\b(?:IOP|IP|OP|IOPU|IOPA|IOPAU|IOPR|IOPUR|IOPAR|IOPX|IOPXU|IOPP|IOPQ|IOPZ|IOPZU|IOPAP|IOPRU)\s*\(\s*"([^"]+)"\s*,\s*(\w+)', allc):
    names.setdefault(m.group(1).lower(), m.group(2))
field = {}  # const -> field
for m in re.finditer(r'case\s+(\w+)\s*:\s*(?:/\*.*?\*/\s*)?(?:\w+->)?(\w+)\s*=\s*value->', allc, re.S):
    field.setdefault(m.group(1), m.group(2))
default = {}
for m in re.finditer(r'if\s*\(\s*!\s*(?:\w+->)?(\w+?)Given\s*\)\s*\{?\s*(?:\w+->)?\1\s*=\s*([^;]+);', allc):
    default.setdefault(m.group(1), m.group(2).strip())
v = open(va).read()
vpar = {}
for m in re.finditer(r'parameter\s+(?:real|integer|string)\s+(\w+)\s*=\s*([^;]+?)\s*(?:from[^;]*)?;', v):
    vpar[m.group(1).lower()] = m.group(2).strip()
for m in re.finditer(r'aliasparam\s+(\w+)\s*=\s*(\w+)\s*;', v):
    vpar.setdefault(m.group(1).lower(), '=' + m.group(2).lower())
rows = []
for n, c in sorted(names.items()):
    f = field.get(c, '?')
    d = default.get(f, '-')
    vn = n if n in vpar else ('model_' + n if 'model_' + n in vpar else None)
    vd = vpar.get(vn) if vn else None
    rows.append((n, f, d, vn or 'ABSENT', vd if vd is not None else ''))
for n in sorted(vpar):
    if n not in names and not (n.startswith('model_') and n[6:] in names):
        rows.append((n, '', '', n, vpar[n] + '  (VA only)'))
w = max(len(r[0]) for r in rows)
for r in rows:
    print(f'{r[0]:{w}}  C {r[1]:28} default {r[2]:18} | VA {r[3]:14} {r[4]}')
