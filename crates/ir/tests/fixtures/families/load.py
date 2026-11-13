"""load.py FAMILY: writes a family's load decks beside it, one ngspice deck (.cir) and one VACASK
deck (.sim) per (condition, region, reading). Families: mos1, mos2, mos3, mos6, mos9 (a bulk at
-0.5 V), jfet1, jfet2, mes1 (three terminals), passive. Conditions: `nom` (27 C, m=1) is the control of
`off` (80 C, m=3); geometry is given in both. Readings: `dc` the operating-point current of `vd`,
`acg` |i(vd)| at 1 GHz with the gate driven, `acd` |i(vd)| at 1 GHz with the drain driven.
`load.py passive` writes the resistor, capacitor and inductor decks (PASSIVES).
The rows of a family are its model cards, its instance lines and its region biases, below.
Both decks set reltol=1e-9 and abstol=1e-15: at VACASK's default abstol a cutoff current of
2.5 pA settles 0.19 pA off (mos1 nom_cut_dc: -2.693e-12 against ngspice-45's -2.502e-12, both
-2.502e-12 at abstol 1e-15), the solver's tolerance and not the device's law."""
import sys

# every MOS level: junctions (cj, cjsw over ad/as/pd/ps) and Meyer capacitances (tox, cgso/cgdo/cgbo)
CAPS = ('tox=20n cgso=3e-10 cgdo=3e-10 cgbo=2e-10 cj=5e-4 mj=0.5 cjsw=2e-10 mjsw=0.33 pb=0.8 js=1e-4 '
        'fc=0.5 rd=10 rs=10')
# level -> its own parameters (each name checked present in both builds' parameter tables)
MOS = {
    1: 'vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 lambda=0.02',
    2: 'vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 lambda=0.02 nsub=1e16 ucrit=1e4 uexp=0.1 vmax=5e4 '
       'neff=1 xj=0.5u nfs=1e11 delta=0.5 ld=0.1u',
    3: 'vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 nsub=1e16 vmax=5e4 xj=0.5u nfs=1e11 theta=0.05 eta=0.1 '
       'kappa=0.2 delta=0.5 ld=0.1u',
    6: 'vto=0.7 kv=2 nv=0.8 kc=5e-5 nc=1.2 lambda0=0.02 lambda1=0.01 gamma=0.5 gamma1=0.1 '
       'sigma=0.01 phi=0.7',
    9: 'vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 nsub=1e16 vmax=5e4 xj=0.5u nfs=1e11 theta=0.05 eta=0.1 '
       'kappa=0.2 delta=0.5 ld=0.1u',
}
FAMILIES = {
    f'mos{k}': dict(
        load=f'spice/mos{k}.osdi',
        ng_model=f'.model nm nmos level={k} {{p}}',
        va_model=f'model nm sp_mos{k} type=1 {{p}}',
        params=f'{own} {CAPS}',
        geometry='w=10u l=2u ad=20p as=20p pd=24u ps=24u',
        nodes='d g 0 b',
        # region: (vgs, vds); the bulk at -0.5 V throughout
        regions={'cut': (0.3, 2.0), 'lin': (3.0, 0.2), 'sat': (1.5, 3.0)},
    )
    for k, own in MOS.items()
}
# the JFETs and the MESFET: three terminals, an area for the geometry; region (vgs, vds)
FAMILIES['jfet1'] = dict(
    load='spice/jfet1.osdi', letter='j', nodes='d g 0', geometry='area=2',
    ng_model='.model nm njf level=1 {p}', va_model='model nm sp_jfet1 type=1 {p}',
    params='vt0=-2 beta=1e-4 lambda=0.02 rd=10 rs=10 cgs=2p cgd=1p pb=0.8 is=1e-14 n=1 fc=0.5 '
           'b=0.9 tcv=2e-3 bex=-1.5 xti=3 eg=1.11',
    regions={'cut': (-2.5, 2.0), 'lin': (0.0, 0.2), 'sat': (-1.0, 3.0)})
FAMILIES['jfet2'] = dict(
    load='spice/jfet2.osdi', letter='j', nodes='d g 0', geometry='area=2',
    ng_model='.model nm njf level=2 {p}', va_model='model nm sp_jfet2 type=1 {p}',
    params='vt0=-2 beta=1e-4 lambda=0.02 rd=10 rs=10 cgs=2p cgd=1p is=1e-14 n=1 fc=0.5 delta=0.1 '
           'xi=1000 p=2 q=2 z=1 vbi=1 hfeta=0.05',
    regions={'cut': (-2.5, 2.0), 'lin': (0.0, 0.2), 'sat': (-1.0, 3.0)})
FAMILIES['mes1'] = dict(
    load='spice/mes1.osdi', letter='z', nodes='d g 0', geometry='area=2',
    ng_model='.model nm nmf level=1 {p}', va_model='model nm sp_mes1 type=1 {p}',
    params='vt0=-1 alpha=2 beta=2.5e-3 lambda=0.05 b=0.3 rd=5 rs=5 cgs=1p cgd=0.5p pb=0.8 is=1e-14 '
           'fc=0.5',
    regions={'cut': (-1.5, 2.0), 'lin': (0.0, 0.2), 'sat': (-0.5, 3.0)})
CONDITIONS = {'nom': (27, 1), 'off': (80, 3)}

# the passives: one region each, so per (condition, form) one reading of `vd` driving node a; an
# inductor stands behind a 1-ohm resistor (a source across it alone has no operating point).
# (ngspice model and instance, VACASK load, model and instance); {m} is the multiplier's text
PASSIVES = {
    'r_value': ('dc', '', 'r1 a 0 1k tc1=1e-3 tc2=1e-5{m}', 'resistor', 'model r sp_resistor',
                'r1 (a 0) r r=1k tc1=1e-3 tc2=1e-5{m}'),
    'r_geometry': ('dc', '.model rm r rsh=100 narrow=0.1u short=0.2u tc1r=1e-3 tc2r=1e-5',
                   'r1 a 0 rm l=10u w=2u{m}', 'resistor',
                   'model rm sp_resistor rsh=100 narrow=0.1u short=0.2u tc1r=1e-3 tc2r=1e-5',
                   'r1 (a 0) rm l=10u w=2u{m}'),
    'c_value': ('ac', '', 'c1 a 0 1p tc1=1e-3 tc2=1e-5{m}', 'capacitor', 'model c sp_capacitor',
                'c1 (a 0) c c=1p tc1=1e-3 tc2=1e-5{m}'),
    'c_geometry': ('ac', '.model cm c cj=1e-3 cjsw=1e-9 narrow=0.1u short=0.2u tc1=1e-3 tc2=1e-5',
                   'c1 a 0 cm w=10u l=20u{m}', 'capacitor',
                   'model cm sp_capacitor cj=1e-3 cjsw=1e-9 narrow=0.1u short=0.2u model_tc1=1e-3 '
                   'model_tc2=1e-5', 'c1 (a 0) cm w=10u l=20u{m}'),
    'l_value': ('ac', 'rs a b 1', 'l1 b 0 1u tc1=1e-3 tc2=1e-5{m}', 'inductor',
                'model l sp_inductor\nmodel r sp_resistor\nrs (a b) r r=1',
                'l1 (b 0) l l=1u tc1=1e-3 tc2=1e-5{m}'),
    'l_geometry': ('ac', '.model lm l csect=1e-6 length=1e-3 mu=100 tc1=1e-3 tc2=1e-5\nrs a b 1',
                   'l1 b 0 lm nt=10{m}', 'inductor',
                   'model lm sp_inductor csect=1e-6 length=1e-3 mu=100 model_tc1=1e-3 '
                   'model_tc2=1e-5\nmodel r sp_resistor\nrs (a b) r r=1', 'l1 (b 0) lm nt=10{m}'),
}


def decks(name, f):
    for cond, (temp, m) in CONDITIONS.items():
        for region, (vg, vd) in f['regions'].items():
            for reading in ('dc', 'acg', 'acd'):
                stem = f'{cond}_{region}_{reading}'
                bulk = f['nodes'].endswith(' b')
                vbs = ', vbs=-0.5' if bulk else ''
                title = f'{name} {cond} ({temp} C, m={m}), {region} (vgs={vg}, vds={vd}{vbs}), {reading}'
                x = f.get('letter', 'm') + '1'
                gac = ' ac 1' if reading == 'acg' else ''
                dac = ' ac 1' if reading == 'acd' else ''
                inst = f['geometry'] + (f' m={m}' if m != 1 else '')
                ng = [title, f['ng_model'].format(p=f['params']),
                      f'vd d 0 dc {vd}{dac}', f'vg g 0 dc {vg}{gac}'] + ['vb b 0 dc -0.5'] * bulk + [
                      f'{x} {f["nodes"]} nm {inst}', f'.temp {temp}', '.options reltol=1e-9 abstol=1e-15',
                      '.control', 'set numdgt=12']
                ng += ['op', 'print i(vd)'] if reading == 'dc' else \
                      ['ac lin 1 1g 1g', 'print mag(i(vd))']
                ng += ['.endc', '.end']
                va_inst = f['geometry'] + (f' $mfactor={m}' if m != 1 else '')
                va = [title, f'load "{f["load"]}"', 'model vs vsource',
                      f['va_model'].format(p=f['params']),
                      f'vd (d 0) vs dc={vd}' + (' mag=1' if dac else ''),
                      f'vg (g 0) vs dc={vg}' + (' mag=1' if gac else ''),
                      ] + ['vb (b 0) vs dc=-0.5'] * bulk + [
                      f'{x} ({f["nodes"]}) nm {va_inst}',
                      'control', f'  options temp={temp} tnom=27 reltol=1e-9 abstol=1e-15 rawfile="ascii"',
                      '  analysis op1 op' if reading == 'dc' else
                      '  analysis ac1 ac from=1e9 to=1e9 mode="lin" points=1', 'endc']
                yield stem, '\n'.join(ng) + '\n', '\n'.join(va) + '\n'
                if (cond, region, reading) == ('nom', 'sat', 'dc'):
                    yield from probes(stem, vg, vd, '\n'.join(ng) + '\n', '\n'.join(va) + '\n')


def probes(stem, vg, vd, ng, va):
    """The oracle of an AC reading: the operating point moved by 1e-4 V either way on the gate
    (`_fd_gp`, `_fd_gm`) and the drain (`_fd_dp`, `_fd_dm`), whose central differences are gm and
    gds of the DC law both simulators share; and the AC readings at 1 kHz (`_acg_1k`, `_acd_1k`),
    where the capacitances add under 1e-8 relative."""
    h = 1e-4
    for tag, (g, d) in {'gp': (vg + h, vd), 'gm': (vg - h, vd), 'dp': (vg, vd + h),
                        'dm': (vg, vd - h)}.items():
        yield (f'{stem}_fd_{tag}',
               ng.replace(f'vd d 0 dc {vd}', f'vd d 0 dc {d!r}').replace(f'vg g 0 dc {vg}',
                                                                        f'vg g 0 dc {g!r}'),
               va.replace(f'vd (d 0) vs dc={vd}', f'vd (d 0) vs dc={d!r}')
                 .replace(f'vg (g 0) vs dc={vg}', f'vg (g 0) vs dc={g!r}'))
    for drive, src in (('acg', 'vg'), ('acd', 'vd')):
        v = vg if src == 'vg' else vd
        n = ng.replace(f'{src} {src[1]} 0 dc {v}', f'{src} {src[1]} 0 dc {v} ac 1')
        a = va.replace(f'{src} ({src[1]} 0) vs dc={v}', f'{src} ({src[1]} 0) vs dc={v} mag=1')
        n = n.replace('op\nprint i(vd)', 'ac lin 1 1k 1k\nprint mag(i(vd))')
        a = a.replace('analysis op1 op', 'analysis ac1 ac from=1e3 to=1e3 mode="lin" points=1')
        yield f'nom_sat_{drive}_1k', n, a


def passive_decks():
    for form, (reading, ng_model, ng_inst, va_load, va_model, va_inst) in PASSIVES.items():
        for cond, (temp, m) in CONDITIONS.items():
            title = f'passive {form} {cond} ({temp} C, m={m}), {reading} (1 V on a, 1 MHz)'
            ac = ' ac 1' if reading == 'ac' else ''
            ng = [title] + ([ng_model] if ng_model else []) + [
                f'vd a 0 dc 1{ac}', ng_inst.format(m=f' m={m}' if m != 1 else ''),
                f'.temp {temp}', '.options reltol=1e-9 abstol=1e-15', '.control', 'set numdgt=12']
            ng += ['op', 'print i(vd)'] if reading == 'dc' else \
                  ['ac lin 1 1meg 1meg', 'print mag(i(vd))']
            ng += ['.endc', '.end']
            va = [title, f'load "spice/{va_load}.osdi"']
            va += ['load "spice/resistor.osdi"'] if 'sp_resistor' in va_model and va_load != 'resistor' else []
            va += ['model vs vsource', va_model, 'vd (a 0) vs dc=1' + (' mag=1' if ac else ''),
                   va_inst.format(m=f' $mfactor={m}' if m != 1 else ''), 'control',
                   f'  options temp={temp} tnom=27 reltol=1e-9 abstol=1e-15 rawfile="ascii"',
                   '  analysis op1 op' if reading == 'dc' else
                   '  analysis ac1 ac from=1e6 to=1e6 mode="lin" points=1', 'endc']
            yield f'{form}_{cond}', '\n'.join(ng) + '\n', '\n'.join(va) + '\n'


name = sys.argv[1]
rows = passive_decks() if name == 'passive' else decks(name, FAMILIES[name])
for stem, ng, va in rows:
    open(f'{name}/{stem}.cir', 'w').write(ng)
    open(f'{name}/{stem}.sim', 'w').write(va)
