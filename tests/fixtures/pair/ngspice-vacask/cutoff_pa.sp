cutoff_pa: a level-1 NMOS in cutoff (vgs=0.3, vds=2, vbs=-0.5) at default tolerances: its drain current is a few pA (ngspice-45 -2.502e-12 A at any abstol; VACASK -2.693e-12 at its default abstol, -2.502e-12 at 1e-15: the IR owner's nom_cut_dc). Proves step 4's absolute current term: the 7 % gap is solver tolerance, inside 2 * (RELTOL * swing + ABSTOL + GMIN * Vmax)
.model nm nmos level=1 vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 lambda=0.02 rd=10 rs=10 tox=20n cgso=3e-10 cgdo=3e-10 cgbo=2e-10 cj=5e-4 mj=0.5 cjsw=2e-10 mjsw=0.33 pb=0.8 js=1e-4 fc=0.5
vd d 0 dc 2.0
vg g 0 dc 0.3
vb b 0 dc -0.5
m1 d g 0 b nm w=10u l=2u ad=20p as=20p pd=24u ps=24u
.op
.end
