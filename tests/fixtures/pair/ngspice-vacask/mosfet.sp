mosfet: an NMOS (SPICE level 1 model) with a resistor load, its drain swept by the gate
vdd vdd 0 dc 3
vg g 0 dc 0
rd vdd d 10k
m1 d g 0 0 nmod w=10u l=2u
.model nmod nmos level=1 vto=0.7 kp=100u lambda=0.02
.dc vg 0 3 0.1
.end
