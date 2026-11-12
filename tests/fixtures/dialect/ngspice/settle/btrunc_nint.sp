trunc settle: nint in a B source
* settle (passes' trunc_to_ternary, step2-passes 6a340ea0): -b: v(n1) = -2.00000e+00 (nint(-2.5) = -2: half to even)
v0 a 0 dc 1
b1 n1 0 v=nint(-2.5*v(a))
r1 n1 0 1k
.control
op
print v(n1)
.endc
.end
