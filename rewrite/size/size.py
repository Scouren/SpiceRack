"""size.py REV: the size table of rewrite/SIZE.md at REV, read from git objects (no checkout).

Rows: NEW crates/ir/src, src/dialect, src/passes; OLD src/syntax split into the files the new
pipeline still uses and the rest. Columns: files, lines, code lines. Test files (a name holding
"test") are left out of every row. Code lines are check_style.py's definition (a line that is not
blank and does not start with `//`, before a file's `#[cfg(test)]` item), read with its own
`rs_items`. "Used" is a reachability closure over module references from the new pipeline's
code (src/dialect and src/passes, the facade's old-route switch and test files apart): edges
are `crate::`, `super::`, `self::` and bare child-module paths, brace imports expanded,
comments stripped; a `mod x;` declaration is no use. Limits: inline test modules other than a
file's `#[cfg(test)]` tail count as code; a glob `use x::*` reaches the module named only;
macro-made paths are not seen. Writes size/used.txt and size/dead.txt beside this script.

Run from the repository root: `python3 rewrite/size/size.py ir-spine`.
"""
import importlib.util, os, re, subprocess, sys
rev = sys.argv[1]
here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('check_style', '.github/scripts/check_style.py')
style = importlib.util.module_from_spec(spec); spec.loader.exec_module(style)
def code_lines(f):
    lines = text[f].split('\n')
    items = list(style.rs_items(lines))
    test_line = next((n for n, r in items if r == style.TEST), len(lines) + 1)
    return sum(1 for l in lines[:test_line - 1] if l.strip() and not l.lstrip().startswith('//'))
def git(*a): return subprocess.run(['git', *a], capture_output=True, text=True, check=True).stdout
files = [f for f in git('ls-tree', '-r', '--name-only', rev, '--', 'src').split() if f.endswith('.rs')]
text = {f: git('show', f'{rev}:{f}') for f in files}
def modpath(f):  # src/syntax/render/read/x.rs -> ['syntax','render','read','x']; mod.rs -> its dir
    p = f[len('src/'):-3].split('/')
    return p[:-1] if p[-1] in ('mod', 'lib') else p
mods = {tuple(modpath(f)): f for f in files}
def resolve(base, path):
    parts = list(path)
    while parts:
        if tuple(parts) in mods: return mods[tuple(parts)]
        parts.pop()
    return None
def expand(src):
    """`a::{b, c::d}` -> `a::b a::c::d` (nested braces, innermost first), so each path is visible."""
    pat = re.compile(r'((?:\w+::)+)\{([^{}]*)\}')
    for _ in range(10):
        new = pat.sub(lambda m: ' '.join(m.group(1) + x.strip() for x in m.group(2).split(',') if x.strip()), src)
        if new == src: break
        src = new
    return src
def refs(f):
    me = modpath(f); out = set()
    text_f = expand(re.sub(r'//[^\n]*', '', text[f]))
    is_mod = f.endswith(('mod.rs', 'lib.rs'))
    here = me if is_mod else me[:-1] + [me[-1]]
    parent = me[:-1] if is_mod else me[:-1]
    for m in re.finditer(r'\bcrate::((?:\w+::)*\w+)', text_f):
        r = resolve(None, m.group(1).split('::'))
        if r: out.add(r)
    for m in re.finditer(r'\bsuper::((?:\w+::)*\w+)', text_f):
        base = (me[:-1] if not is_mod else me[:-1])
        r = resolve(None, base + m.group(1).split('::')) if not is_mod else resolve(None, me[:-1] + m.group(1).split('::'))
        if r: out.add(r)
    for m in re.finditer(r'\bself::((?:\w+::)*\w+)', text_f):
        r = resolve(None, (me if is_mod else me[:-1]) + m.group(1).split('::'))
        if r: out.add(r)
    # a bare path through a child module (`child::x`, edition 2018 paths): the file's own
    # children (a `mod.rs`'s directory, or `x.rs`'s `x/`)
    for path, g in mods.items():
        if len(path) == len(me) + 1 and list(path[:-1]) == me:
            for m in re.finditer(r'(?<![\w:])' + path[-1] + r'::((?:\w+::)*\w+)', text_f):
                r = resolve(None, list(path) + m.group(1).split('::'))
                if r: out.add(r)
    # a non-mod file's children live in its own directory
    return out
# the new pipeline as it would run with the facade routed through it: its code, not the facade's
# old-route switch (src/dialect/facade.rs) and not its tests
test = lambda f: 'test' in f.rsplit('/', 1)[-1]
seeds = [f for f in files if f.startswith(('src/dialect/', 'src/passes/'))
         and f != 'src/dialect/facade.rs' and not test(f)]
seen, work = set(seeds), list(seeds)
while work:
    f = work.pop()
    for r in refs(f):
        if r not in seen:
            seen.add(r); work.append(r)
def row(name, fs):
    fs = [f for f in fs if not test(f)]
    lines = sum(text[f].count('\n') for f in fs)
    print(f'| {name} | {len(fs)} | {lines:,} | {sum(code_lines(f) for f in fs):,} |')
under = lambda p: [f for f in files if f.startswith(p)]
irfiles = [f for f in git('ls-tree', '-r', '--name-only', rev, '--', 'crates/ir/src').split() if f.endswith('.rs')]
for f in irfiles:
    text[f] = git('show', f'{rev}:{f}')
syn = under('src/syntax/')
used = sorted(f for f in syn if f in seen and not test(f))
dead = sorted(f for f in syn if f not in seen and not test(f))
print(f'rev {git("rev-parse", "--short", rev).strip()}')
print('| area | files | lines | code lines |')
print('|---|---|---|---|')
row('NEW crates/ir/src', irfiles)
row('NEW src/dialect', under('src/dialect/'))
row('NEW src/passes', under('src/passes/'))
row('NEW total', irfiles + under('src/dialect/') + under('src/passes/'))
row('OLD src/syntax', syn)
row('OLD src/syntax, used by the new pipeline', used)
row('OLD src/syntax, dead once the facade routes through the new pipeline', dead)
open(os.path.join(here, 'used.txt'), 'w').write('\n'.join(used) + '\n')
open(os.path.join(here, 'dead.txt'), 'w').write('\n'.join(dead) + '\n')
