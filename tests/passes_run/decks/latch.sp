latch: two CMOS inverters a->b->c, c fed back to a by rf=20k, vin into a by rin; nested .dc (inner vin 2..5, outer rin 10k, 10.1k); ngspice-45 v(c) outer 2: 3.490099 2.5 1.509901 0.519802 4.991754 4.995879 5 (each outer step from the initial guess)
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
.control
run
print v(c)
.endc
.end
