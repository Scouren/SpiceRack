inn: an include of an include
* settle (R71 case 6): -b: i(v1) = -5.00000e-04: a subcircuit two includes down binds
.include inc_nested_a.inc
V1 n1 0 1
X1 n1 sub
.control
op
print i(v1)
.endc
.end
