trunc settle: int in a B source
* settle (passes' trunc_to_ternary, step2-passes 6a340ea0): -b: "Error: no such function 'int' at line 4", then a fatal error, exit 1: the B-source parser has no int
v0 a 0 dc 1
b1 n1 0 v=int(-2.5*v(a))
r1 n1 0 1k
.control
op
print v(n1)
.endc
.end
