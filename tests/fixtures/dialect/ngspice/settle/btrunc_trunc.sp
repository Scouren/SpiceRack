trunc settle: trunc in a B source
* settle (passes' trunc_to_ternary, step2-passes 6a340ea0): -b: "Error: no such function 'trunc' at line 4", then a fatal error, exit 1: the B-source parser has no trunc
v0 a 0 dc 1
b1 n1 0 v=trunc(-2.5*v(a))
r1 n1 0 1k
.control
op
print v(n1)
.endc
.end
