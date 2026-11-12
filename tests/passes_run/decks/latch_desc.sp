latch_desc: the latch, outer vin 5 -> 2 V, inner vx on a node of its own (2 points); ngspice-45 v(c): 5 5, 4.991727 x2, 1.5 x2, 3.5 x2 (vin = 3 and 2 on the middle state: afresh is the initial guess)
vdd vdd 0 dc 5
vin in 0 dc 0
rin in a 10k
rf c a 20k
m1p b a vdd vdd pm w=10u l=1u
m1n b a 0 0 nm w=10u l=1u
m2p c b vdd vdd pm w=10u l=1u
m2n c b 0 0 nm w=10u l=1u
vx x 0 dc 0
rx x 0 1k
.model nm nmos level=1 vto=1 kp=1e-4
.model pm pmos level=1 vto=-1 kp=1e-4
.dc vx 0 1 1 vin 5 2 -1
.control
run
print v(c)
.endc
.end
