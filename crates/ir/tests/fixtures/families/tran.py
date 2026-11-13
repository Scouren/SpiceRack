"""tran.py: writes tran/<family>.{cir,sim}: one transient per family whose small-signal is a law
point (R94, R94a, R95: mos2, mos3, mos6, mos9; bjt with irb; vdmos_load at m=3, F30), the charges in play, on both
simulators, at two step caps: span/2000 (20 ps over 40 ns, R84) in tran/ and 5 ps in
tran_fine/. A gate (base) driven by a pulse (0..3 V, a base 0..5 V) through 10 kohm, the drain (collector) loaded by 2 kohm
from 3 V, the device on load.py's off-nominal cards (80 C, m=3, geometry given). Reading: v(d) at TIMES (ngspice `meas
... find v(d) at=`, VACASK's raw interpolated by tranread.py)."""
import importlib.util, os, sys

here = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location('load', os.path.join(here, 'load.py'))
TIMES = ['2n', '3n', '4n', '6n', '8n', '9n', '12n', '20n', '30n']


def write(name, cap, out, m=3, stem=None):
    """m: 3 (the instance's multiplier), 1, or 'x3' (three copies at m=1, the m relation's oracle)."""
    load = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(load)
    f = load.FAMILIES[name]
    ng_m, va_m = (' m=3', ' $mfactor=3') if m == 3 else ('', '')
    bulk = f['nodes'].endswith(' b')
    high = 3 if name.startswith('mos') or name == 'bsim3' else 5
    # the 1 V BSIM4 card: a 1 V supply and drive
    vdd = 1 if name.startswith('bsim4') else 3
    high = 1 if name.startswith('bsim4') else high
    # vdmos_load: cgs 1n, so 10 ohm puts its gate's time constant at 10 ns like the others' few ns
    rg = '10' if name == 'vdmos_load' else '10k'
    tol = f.get('abstol', '1e-15')
    letter = f.get('letter', 'm')
    va_nodes = f.get('va_nodes', f['nodes'])
    # each copy its own internal (thermal) nodes: VACASK's extra terminals beyond the device's own
    copies = [1, 2, 3] if m == 'x3' else [1]
    own = len(f['nodes'].split())
    va_inst = [f'{letter}{k} (' + ' '.join(n if j < own else f'{n}{k}' for j, n in enumerate(va_nodes.split()))
               + f') nm {f["geometry"]}{va_m}' for k in copies]
    ng = [f'{name} transient, 80 C, m={m}, pulse 0..{high} V on the gate through {rg}, 2k load from {vdd} V',
          f['ng_model'].format(p=f['params']), f'vdd dd 0 dc {vdd}', 'rl dd d 2k',
          f'vin in 0 dc 0 pulse(0 {high} 1n 1n 1n 5n 20n)', f'rg in g {rg}']
    ng += ['vb b 0 dc -0.5'] * bulk + [f'{letter}{k} {f["nodes"]} nm {f["geometry"]}{ng_m}' for k in copies] + ['.temp 80',
                                       f'.options reltol=1e-6 abstol={tol}', '.control', 'set numdgt=12',
                                       f'tran {cap / 2:g}p 40n 0 {cap:g}p']
    ng += [f'meas tran t{t} find v(d) at={t}' for t in TIMES] + ['.endc', '.end']
    va = [ng[0], f'load "{f["load"]}"', 'load "spice/resistor.osdi"', 'model vs vsource',
          'model r sp_resistor', f['va_model'].format(p=f['params']), f'vdd (dd 0) vs dc={vdd}',
          'rl (dd d) r r=2k',
          f'vin (in 0) vs type="pulse" val0=0 val1={high} delay=1n rise=1n fall=1n width=5n period=20n',
          f'rg (in g) r r={rg}']
    va += ['vb (b 0) vs dc=-0.5'] * bulk + va_inst + ['control',
        f'  options temp=80 tnom=27 reltol=1e-6 abstol={tol} rawfile="ascii"',
        f'  analysis tran1 tran step={cap / 2:g}p stop=40n maxstep={cap:g}p', 'endc']
    open(f'{here}/{out}/{stem or name}.cir', 'w').write('\n'.join(ng) + '\n')
    open(f'{here}/{out}/{stem or name}.sim', 'w').write('\n'.join(va) + '\n')


# the step cap at span/2000 (R84) and a quarter of it: a step-control difference shrinks with it
for cap, out in ((20, 'tran'), (5, 'tran_fine')):
    os.makedirs(f'{here}/{out}', exist_ok=True)
    for n in ['mos1', 'mos2', 'mos3', 'mos6', 'mos9', 'bjt', 'bjt_rbconst', 'vdmos_load', 'bsim3',
              'bsim4', 'bsim4_nf']:
        write(n, cap, out)
    # F30's control: the same VDMOS at m=1
    write('vdmos_load', cap, out, m=1, stem='vdmos_m1')
    write('vdmos_load', cap, out, m='x3', stem='vdmos_x3')
