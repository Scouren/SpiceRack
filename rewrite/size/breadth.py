"""breadth.py: code lines in crates/ir/src (working tree) that exist only for a row type no dialect
reader of today builds. Per group, two numbers with check_style.py's code-line definition:
  blocks: the whole items headed by the group's own types (`struct T`, `enum T`, `impl … for T`,
          `impl T`, a `run_fields!`/`fields!` table entry `T {`), which would be deleted with it;
  lines:  other code lines outside those blocks that name one of the group's identifiers as a
          whole word (match arms, ALL lists, generator arms, dispatch rows): an upper bound for
          what goes with it, since a line may also name something else.
Walkers and dispatch macros are not attributed. Run from the repository root:
`python3 rewrite/size/breadth.py`."""
import glob, importlib.util, re
spec = importlib.util.spec_from_file_location('check_style', '.github/scripts/check_style.py')
style = importlib.util.module_from_spec(spec); spec.loader.exec_module(style)
GROUPS = [  # (group, dialects the inventory names for it, its own types, other identifiers)
    ('Analysis Hb (harmonic balance)', 'xy hs es sp va (INV G16, G19)', ['Hb', 'HbTruncate'], ['HbDiamond', 'HbHybrid', 'HbIntermod', 'HbMultiTone', 'PrintHbTime']),
    ('Analyses Pac, Pnoise, Pxf, Pstb (periodic small-signal)', 'sp (G18), va: pac pnoise (G19)', ['Pac', 'Pnoise', 'Pxf', 'Pstb', 'FreqAxis'], ['PeriodicRelative']),
    ('Analysis Net (two-port network)', 'lt hs (G37, H15)', ['Net'], []),
    ('Analysis TimedOp', 'hs sp (G42)', ['TimedOp'], []),
    ('Item Statistics (variations)', 'sp (G17); va mc', ['Statistics', 'Vary', 'VaryDist', 'Correlation'], ['StatsCorrelation', 'StatsMismatch', 'StatsTruncate', 'VaryLogNormal', 'VaryMean', 'VaryPercent']),
    ('Elements Laplace, FreqTable, Delay (and their detail rows)', 'Laplace: lt hs es, ng via XSPICE (D23); Delay: hs, xy, lt (D90); FreqTable: hs (D106)', ['Laplace', 'LaplaceForm', 'FreqTable', 'FreqLaw', 'FreqUnit', 'Delay', 'DelayLaw'], ['LaplaceExprInput', 'LaplacePoleZero', 'FreqComplexLinear', 'FreqDbLogFreq', 'DelaySampled']),
    ('Element FluxInductor', 'lt (D80)', ['FluxInductor'], []),
    ('Element Igbt', 'lt (D103, F22)', ['Igbt'], []),
    ('Element Module (Verilog-A module instance)', 'sp, ng `N` card, va (B24)', ['Module'], []),
    ('Wave Pattern', 'hs xy (E32)', ['Pattern'], ['DcAtZeroPattern']),
]
# finer level: members that are a field, a flag or a table row of a row some reader builds; no
# whole block is theirs, so only the code lines naming one of them qualified (`::Name`) count
MEMBERS = None  # filled from the L7 list when given: `python3 rewrite/size/breadth.py <unspoken.txt>`
files = sorted(glob.glob('crates/ir/src/*.rs'))
def code(l): return bool(l.strip()) and not l.lstrip().startswith('//')
def body(lines):
    items = list(style.rs_items(lines))
    t = next((n for n, r in items if r == style.TEST), len(lines) + 1)
    return lines[:t - 1]
src = {f: body(open(f).read().split('\n')) for f in files}
total_b = total_l = 0
print('| group | planned dialect (inventory) | code lines, whole blocks | other code lines naming it (upper bound) |')
print('|---|---|---|---|')
for name, who, types, idents in GROUPS:
    head = re.compile(r'^\s*(pub(\([^)]*\))?\s+)?(struct|enum)\s+(' + '|'.join(types) + r')\b|^\s*impl\b[^{]*\b(for\s+)?(' + '|'.join(types) + r')\s*(<[^>]*>)?\s*\{|^\s{4}(' + '|'.join(types) + r')\s*\{')
    word = re.compile(r'\b(' + '|'.join(types + idents) + r')\b')
    blocks = lines = 0
    for f, ls in src.items():
        skip = set()
        for i, l in enumerate(ls):
            if head.match(l):
                end = style.rs_block_end(ls, i) if not l.startswith('    ') else next((k for k in range(i, len(ls)) if ls[k].strip().endswith('}') and (k > i or ls[k].count('{') == ls[k].count('}'))), i)
                for k in range(i, min(end, len(ls) - 1) + 1):
                    skip.add(k)
        blocks += sum(1 for k in skip if code(ls[k]))
        lines += sum(1 for k, l in enumerate(ls) if k not in skip and code(l) and word.search(l))
    total_b += blocks; total_l += lines
    print(f'| {name} | {who} | {blocks:,} | {lines:,} |')
print(f'| **total** | | **{total_b:,}** | **{total_l:,}** |')
import sys
if len(sys.argv) > 1:
    kinds = {}
    for line in open(sys.argv[1]):
        m = re.match(r'(\w+)\((\w+)(?:,(\w+))?\)', line.strip())
        if m:
            kinds.setdefault(m.group(1), []).append(m.group(3) or m.group(2))
    print()
    print('| member group (finer level) | members | code lines naming one, qualified `::Name` (upper bound) |')
    print('|---|---|---|')
    for k in sorted(kinds):
        if k == 'Placed':
            print(f'| Placed | {len(kinds[k])} | no code of its own: a (scope kind, item kind) pair the generic feature walk derives |')
            continue
        names = kinds[k]
        pat = re.compile(r'::(' + '|'.join(names) + r')\b')
        n = sum(1 for ls in src.values() for l in ls if code(l) and pat.search(l))
        print(f'| {k} | {len(names)} | {n:,} |')
