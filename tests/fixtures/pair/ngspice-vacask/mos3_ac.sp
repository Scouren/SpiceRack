mos3_ac: F27 (R94): one level-3 NMOS in saturation (vgs 1.5, vds 3, vbs -0.5), its gate driven at 1 kHz: ngspice-45 small-signal gm is not the derivative of its own DC law, VACASK sp_mos3 is; DC agrees, the drain current at 1 kHz does not (the IR owner's families/mos3/nom_sat_acg_1k)
.model nm nmos level=3 vto=0.7 kp=1e-4 gamma=0.5 phi=0.7 nsub=1e16 vmax=5e4 xj=0.5u nfs=1e11 theta=0.05 eta=0.1 kappa=0.2 delta=0.5 ld=0.1u tox=20n cgso=3e-10 cgdo=3e-10 cgbo=2e-10 cj=5e-4 mj=0.5 cjsw=2e-10 mjsw=0.33 pb=0.8 js=1e-4 fc=0.5 rd=10 rs=10
vd d 0 dc 3.0
vg g 0 dc 1.5 ac 1
vb b 0 dc -0.5
m1 d g 0 b nm w=10u l=2u ad=20p as=20p pd=24u ps=24u
.ac lin 1 1k 1k
.end
