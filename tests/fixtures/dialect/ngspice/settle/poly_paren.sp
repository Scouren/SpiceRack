poly_paren
* settle -b: v(o1) = v(o2) = 1.400000e+01, v(o3) = v(o4) = 8.128340e-05: POLY coefficients written (1, 2, 3) or (0,2.42834E-5,3.8E-5) are the same as written apart (TransImpedanceAmp/output.sp F1)
v1 a 0 dc 2
v2 b 0 dc 3
vs1 a1 a2 dc 0
vs2 b1 b2 dc 0
ra a a1 1
ra2 a2 0 1
rb b b1 1
rb2 b2 0 1
e1 o1 0 poly(2) a 0 b 0 (1, 2, 3)
e2 o2 0 poly(2) a 0 b 0 1 2 3
f1 0 o3 poly(2) vs1 vs2 (0,2.42834E-5,3.8E-5)
f2 0 o4 poly(2) vs1 vs2 0 2.42834E-5 3.8E-5
r1 o1 0 1
r2 o2 0 1
r3 o3 0 1
r4 o4 0 1
.control
op
print v(o1) v(o2) v(o3) v(o4) i(vs1) i(vs2)
.endc
.end
