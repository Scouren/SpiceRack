wanalysis1: an analysis field (stop 3 gives 4 points, stop 1 gives 2): {a}+{b}
* settle (audit row 4): -b: 'Error on line 6 or its substitute:', '.dc v1 0    1.000000000000000e+00    +    2.000000000000000e+00     1', 'Bad syntax!', 'Simulation interrupted due to error!' (invalid in source)
.param a=1 b=2
V1 n 0 1
R1 n 0 1
.dc v1 0 {a}+{b} 1
.control
set numdgt=17
run
print length(v(n))
.endc
.end
