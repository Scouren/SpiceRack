latch_vdd: F8 (R75), the form that reaches the simulators today: latch.sp with its outer axis on the supply (vdd 5, 5.1) instead of rin, which no rule converts yet
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
.dc vin 2 5 0.5 vdd 5 5.1 0.1
.end
