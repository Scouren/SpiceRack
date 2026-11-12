mosfet_op: the operating point of an NMOS and a PMOS (SPICE level 1 models) with resistor loads: the model family converts without a sweep
vdd vdd 0 dc 3
vg g 0 dc 1.5
rn vdd dn 10k
m1 dn g 0 0 nmod w=10u l=2u
rp dp 0 10k
m2 dp g vdd vdd pmod w=20u l=2u
.model nmod nmos level=1 vto=0.7 kp=100u lambda=0.02
.model pmod pmos level=1 vto=-0.7 kp=50u lambda=0.02
.op
.end
