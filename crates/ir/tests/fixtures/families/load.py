"""load.py FAMILY: writes a family's load decks beside it, one ngspice deck (.cir) and one VACASK
deck (.sim) per (condition, region, reading). Conditions: `nom` (27 C, m=1) is the control of
`off` (80 C, m=3); geometry is given in both. Readings: `dc` the operating-point current of `vd`,
`acg` |i(vd)| at 1 GHz with the gate driven, `acd` |i(vd)| at 1 GHz with the drain driven.
`load.py passive` writes the resistor, capacitor and inductor decks (PASSIVES).
The rows of a family are its model cards, its instance lines and its region biases, below.
Both decks set reltol=1e-9 and abstol=1e-15: at VACASK's default abstol a cutoff current of
2.5 pA settles 0.19 pA off (mos1 nom_cut_dc: -2.693e-12 against ngspice-45's -2.502e-12, both
-2.502e-12 at abstol 1e-15), the solver's tolerance and not the device's law."""
import sys

FAMILIES = {
    # mos1: junctions (cj, cjsw over ad/as/pd/ps) and Meyer capacitances (tox, cgso/cgdo/cgbo)
    'mos1': dict(
        load='spice/mos1.osdi',
        ng_model='.model nm nmos level=1 {p}',
        va_model='model nm sp_mos1 type=1 {p}',
        params='vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 lambda=0.02 rd=10 rs=10 tox=20n cgso=3e-10 '
               'cgdo=3e-10 cgbo=2e-10 cj=5e-4 mj=0.5 cjsw=2e-10 mjsw=0.33 pb=0.8 js=1e-4 fc=0.5',
        geometry='w=10u l=2u ad=20p as=20p pd=24u ps=24u',
        nodes='d g 0 b',
        # region: (vgs, vds); the bulk at -0.5 V throughout
        regions={'cut': (0.3, 2.0), 'lin': (3.0, 0.2), 'sat': (1.5, 3.0)},
    ),
}
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
                title = (f'{name} {cond} ({temp} C, m={m}), {region} (vgs={vg}, vds={vd}, vbs=-0.5), '
                         f'{reading}')
                gac = ' ac 1' if reading == 'acg' else ''
                dac = ' ac 1' if reading == 'acd' else ''
                inst = f['geometry'] + (f' m={m}' if m != 1 else '')
                ng = [title, f['ng_model'].format(p=f['params']),
                      f'vd d 0 dc {vd}{dac}', f'vg g 0 dc {vg}{gac}', 'vb b 0 dc -0.5',
                      f'm1 {f["nodes"]} nm {inst}', f'.temp {temp}', '.options reltol=1e-9 abstol=1e-15',
                      '.control', 'set numdgt=12']
                ng += ['op', 'print i(vd)'] if reading == 'dc' else \
                      ['ac lin 1 1g 1g', 'print mag(i(vd))']
                ng += ['.endc', '.end']
                va_inst = f['geometry'] + (f' $mfactor={m}' if m != 1 else '')
                va = [title, f'load "{f["load"]}"', 'model vs vsource',
                      f['va_model'].format(p=f['params']),
                      f'vd (d 0) vs dc={vd}' + (' mag=1' if dac else ''),
                      f'vg (g 0) vs dc={vg}' + (' mag=1' if gac else ''),
                      'vb (b 0) vs dc=-0.5',
                      f'm1 ({f["nodes"]}) nm {va_inst}',
                      'control', f'  options temp={temp} tnom=27 reltol=1e-9 abstol=1e-15 rawfile="ascii"',
                      '  analysis op1 op' if reading == 'dc' else
                      '  analysis ac1 ac from=1e9 to=1e9 mode="lin" points=1', 'endc']
                yield stem, '\n'.join(ng) + '\n', '\n'.join(va) + '\n'


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
