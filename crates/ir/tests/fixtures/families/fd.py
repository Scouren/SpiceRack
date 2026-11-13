"""fd.py: reads load_run.sh's rows on stdin; for each region with `_fd_` probes (`nom_<region>_dc_fd_*`, `nom_<region>_*_1k`), prints per simulator gm and
gds from the central differences of its own operating points (h = 1e-5 V) beside its 1 kHz AC
readings, and each AC reading's relative distance from the difference (the oracle: the derivative
of the DC law, which both simulators agree on to 1e-12). Last, every `m3` reading whose ratio to
its `nom` reading is not 3 within 1e-9 relative and 1e-10 absolute (m copies in parallel; the
solver's gmin is not multiplied). A deck with an element outside the device (the inductors' 1-ohm
resistor) does not hold the relation. The oracle holds where the conductance is
well above the capacitive current at 1 kHz and above the differences' resolution (about 1e-12 S):
not in cutoff, where the 1 kHz reading is the capacitances'."""
import re, sys
rows = {}
regions = []
for line in sys.stdin:
    m = re.match(r'(\S+)\s+ngspice=(\S*)\s+vacask=(\S*)', line)
    if m:
        rows[m.group(1)] = (m.group(2), m.group(3))
        if m.group(1).endswith('_dc_fd_dp'):
            regions.append(m.group(1)[:-len('_dc_fd_dp')])
h = 1e-5
for at, side, k in [(at, side, k) for at in regions for side, k in (('ngspice', 0), ('vacask', 1))]:
    try:
        v = {n[len(at) + 1:]: float(r[k]) for n, r in rows.items()
             if n.startswith(at + '_') and ('_fd_' in n or n.endswith('_1k'))}
        gds = abs(v['dc_fd_dp'] - v['dc_fd_dm']) / (2 * h)
        ad = v['acd_1k']
    except (KeyError, ValueError):
        continue
    rel = lambda a, b: f'{abs(abs(a) - b) / max(b, 1e-300):.2e}'
    gate = ''
    if 'dc_fd_gp' in v:
        gm = abs(v['dc_fd_gp'] - v['dc_fd_gm']) / (2 * h)
        gate = f'gm(fd) {gm:.7e} acg_1k {v.get("acg_1k")} ({rel(v["acg_1k"], gm)});  '
    print(f'{at} {side}: {gate}gds(fd) {gds:.7e} acd_1k {ad} ({rel(ad, gds)})')

# the multiplier's metamorphic relation: every m3 reading is 3 times its nom reading
for n, r in sorted(rows.items()):
    parts = n.split('_')
    if 'm3' not in parts:
        continue
    base = '_'.join('nom' if p == 'm3' else p for p in parts)
    for side, k in (('ngspice', 0), ('vacask', 1)):
        try:
            a, b = float(r[k]), float(rows[base][k])
        except (KeyError, ValueError):
            continue
        # gmin (1e-12 S per junction, never multiplied) carries up to 1e-11 A or S here
        if abs(a - 3 * b) > 1e-9 * abs(a) + 1e-10:
            print(f'm3 {side}: {n} = {a / b if b else float("inf"):.10g} x {base} (3 expected)')
