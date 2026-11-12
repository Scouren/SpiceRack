trunc settle: floor in a B source
* settle (passes' trunc_to_ternary, step2-passes 6a340ea0): -b: v(n1) = -3.00000e+00 (floor(-2.5) = -3)
v0 a 0 dc 1
b1 n1 0 v=floor(-2.5*v(a))
r1 n1 0 1k
.control
op
print v(n1)
.endc
.end
