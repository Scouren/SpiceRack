tran_pwl: the transient of an RC low-pass (tau 10 ns) under a PWL source
v1 in 0 dc 0 pwl(0 0 10n 1 30n 1 40n -0.5 60n 0)
r1 in out 1k
c1 out 0 10p
.tran 0.5n 80n
.end
