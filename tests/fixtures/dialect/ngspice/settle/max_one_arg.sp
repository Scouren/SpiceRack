mo: max with one argument, around a .func call
* settle (the 38 decks, func_cap.sp): -b, numdgt=8: i(v1) = -1.0206207e-01 (R1 = c = f(1,2,3,4) = 2*sqrt(24) = 9.798): numparam's max of one argument is that argument (its first and its last)
.func f(A,B,C,D) '2*A*sqrt(B*C*D)'
.param c='max(f(1,2,3,4))'
V1 a 0 1
R1 a 0 {c}
.control
set numdgt=8
op
print i(v1)
.endc
.end
