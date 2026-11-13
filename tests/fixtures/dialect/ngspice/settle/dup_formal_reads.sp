dup_formal_reads
* settle -b: 'i(v1) = -2.50000e-01' (x1: w = k*2 = 4 from the body), 'i(v2) = -2.00000e-01' (x2's w=5 still wins): the body value reads a body parameter, so it cannot move to the default (read where the instance stands, R53): Unread R1002
.subckt twin 1 2 w=1
.param k=2
.param w={k*2}
r1 1 2 {w}
.ends
x1 n 0 twin
x2 m 0 twin w=5
v1 n 0 1
v2 m 0 1
.control
op
print i(v1) i(v2)
.endc
.end
