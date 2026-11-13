"""fd.py: reads load_run.sh's rows on stdin; for each region with `_fd_` probes (`nom_<region>_dc_fd_*`, `nom_<region>_*_1k`), prints per simulator gm and
gds from the central differences of its own operating points (h = 1e-5 V) beside its 1 kHz AC
readings, and each AC reading's relative distance from the difference (the oracle: the derivative
of the DC law, which both simulators agree on to 1e-12). The oracle holds where the conductance is
well above the capacitive current at 1 kHz and above the differences' resolution (about 1e-12 S):
not in cutoff, where the 1 kHz reading is the capacitances'."""
import re, sys
rows = {}
regions = []
for line in sys.stdin:
    m = re.match(r'(\S+)\s+ngspice=(\S*)\s+vacask=(\S*)', line)
    if m:
        rows[m.group(1)] = (m.group(2), m.group(3))
        if m.group(1).endswith('_dc_fd_gp'):
            regions.append(m.group(1)[:-len('_dc_fd_gp')])
h = 1e-5
for at, side, k in [(at, side, k) for at in regions for side, k in (('ngspice', 0), ('vacask', 1))]:
    try:
        v = {n[len(at) + 1:]: float(r[k]) for n, r in rows.items()
             if n.startswith(at + '_') and ('_fd_' in n or n.endswith('_1k'))}
        gm = abs(v['dc_fd_gp'] - v['dc_fd_gm']) / (2 * h)
        gds = abs(v['dc_fd_dp'] - v['dc_fd_dm']) / (2 * h)
    except (KeyError, ValueError):
        continue
    ag, ad = v.get('acg_1k'), v.get('acd_1k')
    rel = lambda a, b: f'{abs(a - b) / max(abs(b), 1e-300):.2e}' if a is not None else 'none'
    print(f'{at} {side}: gm(fd) {gm:.7e} acg_1k {ag} ({rel(ag, gm)});  gds(fd) {gds:.7e} '
          f'acd_1k {ad} ({rel(ad, gds)})')
