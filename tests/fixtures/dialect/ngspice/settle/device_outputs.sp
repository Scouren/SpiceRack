f13: which device outputs a save names exist
* settle: -b, display: @i1[c], @i1[current], @i1[dc], @r1[i], @v1[i] are 59 long; @i1[i] and @r1[current] are 0 long: a name a device lacks is an empty vector
V1 a 0 dc 1
R1 a b 1k
I1 b 0 dc 1m
.control
save all @i1[c] @i1[current] @v1[i] @r1[i] @i1[i] @r1[current] @i1[dc]
tran 1n 5n
display
write f13.raw
.endc
.end
