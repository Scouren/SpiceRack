import subprocess, re, sys, os
here = os.path.dirname(os.path.abspath(__file__))
card = open(sys.argv[1]).read().split('\n', 1)[1]
for label, params in (('defaults', ''), ('gf180 card', card)):
    for ngv, vav in (('4.5', ''), ('4.5', 'version="4.5"'), ('4.5', 'version="4.8.2"'), ('4.8.2', 'version="4.8.2"'), ('4.8', 'version="4.8.3"')):
        open(f'{here}/v.cir', 'w').write(f".\n.model nm nmos level=54 version={ngv} {params}\nvd d 0 dc 3.3\nvg g 0 dc 1.5\nvb b 0 dc -0.5\nm1 d g 0 b nm w=1u l=0.28u\n.options reltol=1e-9 abstol=1e-15\n.control\nset numdgt=12\nop\nprint i(vd)\n.endc\n.end\n")
        open(f'{here}/v.sim', 'w').write(f'.\nload "spice/bsim4v8.osdi"\nmodel vs vsource\nmodel nm sp_bsim4v8 type=1 {vav} {params}\nvd (d 0) vs dc=3.3\nvg (g 0) vs dc=1.5\nvb (b 0) vs dc=-0.5\nm1 (d g 0 b) nm w=1u l=0.28u\ncontrol\n  options temp=27 tnom=27 reltol=1e-9 abstol=1e-15 rawfile="ascii"\n  analysis op1 op\nendc\n')
        o = subprocess.run(['ngspice', '-b', 'v.cir'], cwd=here, capture_output=True, text=True, timeout=30).stdout
        a = float(re.search(r'^i\(vd\) = (\S+)', o, re.M).group(1))
        vo = subprocess.run(['vacask', 'v.sim'], cwd=here, capture_output=True, text=True, timeout=30)
        b = float(subprocess.run(['python3', sys.argv[2], f'{here}/op1.raw', 'i:vd'], capture_output=True, text=True).stdout)
        warn = [l for l in (vo.stdout + vo.stderr).split('\n') if 'version' in l.lower()]
        print(f'{label:10} ngspice version={ngv:6} vacask {vav or "(none)":18} ngspice={a:.12e} vacask={b:.12e} rel={abs(a-b)/abs(b):.2e} {warn[:1]}')
