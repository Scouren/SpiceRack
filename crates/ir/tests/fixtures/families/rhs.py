"""rhs.py CDIR CFILE VA TAG: per assigned field, the right-hand sides on each side after mapping C parameter
fields to the module's parameter names and normalizing constants/spelling; prints the fields whose RHS sets differ."""
import glob, re, sys
cdir, cfile, va, tag = sys.argv[1:5]
allc = '\n'.join(open(f, errors='replace').read() for f in glob.glob(cdir + '/*.c'))
names = {}
for m in re.finditer(r'\b(?:IO?P\w*|OP\w*)\s*\(\s*"([^"]+)"\s*,\s*(\w+)', allc):
    names.setdefault(m.group(2), m.group(1).lower())
field2name = {}
for m in re.finditer(r'case\s+(\w+)\s*:\s*(?:/\*.*?\*/\s*)?(?:\w+->)?(\w+)\s*=\s*value->', allc, re.S):
    if m.group(1) in names:
        field2name.setdefault(m.group(2), names[m.group(1)])
v = open(va).read()
vparams = set(m.group(1) for m in re.finditer(r'parameter\s+(?:real|integer)\s+(\w+)', v))
for f, n in list(field2name.items()):
    field2name[f] = n if n in vparams else ('model_' + n if 'model_' + n in vparams else n)
def norm(s):
    s = re.sub(r'/\*.*?\*/', '', s, flags=re.S)
    s = re.sub(r'\b(here|model)->', '', s)
    s = re.sub(r'\bREFTEMP\b', '(27.0+273.15)', s)
    s = re.sub(r'\bCONSTboltz\b', '1.38064852e-23', s)
    s = re.sub(r'\bCHARGE\b', '1.6021766208e-19', s)
    s = re.sub(r'\blog\b', 'ln', s)
    s = re.sub(r'\bfabs\b', 'abs', s)
    s = re.sub(r'\bsqrt\b', 'sqrt', s)
    s = re.sub(r'(\d)\.(?!\d)', r'\1.0', s)
    s = re.sub(r'(?<![\w.])\.(\d)', r'0.\1', s)
    s = re.sub(r'\bi_(\w+)', r'\1', s)
    for f, n in field2name.items():
        s = re.sub(r'\b' + f + r'\b', n, s)
    s = re.sub(r'\bTemp\b', 'temp', s)
    # multiplicity: the C instance multiplier against the module's $mfactor
    s = re.sub(r'\*\s*m\b', '', s)
    s = re.sub(r'\bm\s*\*', '', s)
    s = re.sub(r'\bm\s*/', '1.0/', s)
    s = re.sub(r'[\s()]', '', s)
    return s
def assigns(text):
    out = {}
    text = re.sub(r'//[^\n]*', '', text)
    for m in re.finditer(r'(?<![=!<>])\b((?:here->|model->)?[A-Za-z_]\w*)\s*(\*|\+|-|/)?=\s*([^;]+);', text):
        lhs = norm(m.group(1)); op = m.group(2) or ''
        rhs = norm(m.group(3))
        if op:
            rhs = f'{lhs}{op}{rhs}'
        out.setdefault(lhs, set()).add(rhs)
    return out
c = assigns(open(cfile, errors='replace').read())
lines = v.split('\n')
a = next(i for i, l in enumerate(lines) if f'// Tag: {tag},' in l)
b = next((i for i in range(a + 1, len(lines)) if '// Tag:' in lines[i]), len(lines))
vsec = '\n'.join(lines[a:b])
vsec = re.sub(r'\$param_given\((\w+)\)', r'\1Given', vsec)
va_ = assigns(vsec)
for k in sorted(set(c) & set(va_)):
    if c[k] != va_[k] and not k.endswith('_dT'):
        print(f'{k}:\n   C  {sorted(c[k])}\n   VA {sorted(va_[k])}')
