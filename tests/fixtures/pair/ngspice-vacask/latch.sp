latch: F8 (R75): a two-axis DC sweep of a bistable latch (two CMOS inverters a->b->c, c fed back to a through rf); ngspice starts the inner sweep afresh at each outer step, VACASK carries the last solve: at outer step 2 (rin 10.1k), vin 2 V, v(c) is 3.49 V on ngspice and 4.97 V on VACASK
vdd vdd 0 dc 5
vin in 0 dc 0
rin in a 10k
rf c a 20k
m1p b a vdd vdd pm w=10u l=1u
m1n b a 0 0 nm w=10u l=1u
m2p c b vdd vdd pm w=10u l=1u
m2n c b 0 0 nm w=10u l=1u
.model nm nmos level=1 vto=1 kp=1e-4
.model pm pmos level=1 vto=-1 kp=1e-4
.dc vin 2 5 0.5 rin 10k 10.1k 0.1k
.end
