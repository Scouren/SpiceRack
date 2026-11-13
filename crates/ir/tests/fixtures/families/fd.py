"""fd.py: reads load_run.sh's rows on stdin; where the `_fd_` probes stand, prints per simulator gm and
gds from the central differences of its own operating points (h = 1e-4 V) beside its 1 kHz AC
readings, and each AC reading's relative distance from the difference (the oracle: the derivative
of the DC law, which both simulators agree on to 1e-12)."""
import re, sys
rows = {}
for line in sys.stdin:
    m = re.match(r'(\S+)\s+ngspice=(\S*)\s+vacask=(\S*)', line)
    if m:
        rows[m.group(1)] = (m.group(2), m.group(3))
h = 1e-4
for side, k in (('ngspice', 0), ('vacask', 1)):
    try:
        v = {n: abs(float(r[k])) for n, r in rows.items() if '_fd_' in n or n.endswith('_1k')}
        gm = (v['nom_sat_dc_fd_gp'] - v['nom_sat_dc_fd_gm']) / (2 * h)
        gds = (v['nom_sat_dc_fd_dp'] - v['nom_sat_dc_fd_dm']) / (2 * h)
    except (KeyError, ValueError):
        continue
    ag, ad = v.get('nom_sat_acg_1k'), v.get('nom_sat_acd_1k')
    rel = lambda a, b: f'{abs(a - b) / b:.2e}' if a is not None else 'none'
    print(f'{side}: gm(fd) {gm:.7e} acg_1k {ag} ({rel(ag, gm)});  gds(fd) {gds:.7e} '
          f'acd_1k {ad} ({rel(ad, gds)})')
