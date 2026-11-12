save scoped, default ac
v1 in 0 dc 1 ac 1
r1 in out 1k
l1 out mid 1m
d1 mid 0 dm
.model dm d is=1e-14 rs=10
.control
save v(out)
op
save all
ac dec 1 1 10
save v(out)
op
write ng.raw
.endc
.end
