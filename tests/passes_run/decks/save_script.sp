save scoped, default ac, ngspice script
v1 in 0 dc 1 ac 1
r1 in out 1k
l1 out mid 1m
d1 mid 0 dm
.model dm d is=1e-14 rs=10
.control
save v(out)
op
write p1.raw
save all
ac dec 1 1 10
write p2.raw
delete all
save v(out)
op
write p3.raw
.endc
.end
