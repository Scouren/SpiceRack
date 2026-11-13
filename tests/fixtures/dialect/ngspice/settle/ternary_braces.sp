tb: braces inside a ternary of an R value that reads time
* settle (the 38 decks, vdmos/dcdc.sp): -b: i(v1)[1] = -1.00000e-02, i(v1)[58] = -5.00000e-03 at time[58] = 2e-06: numparam replaces the braces, the ternary runs as the R's value (100, then 200 after 1u)
.param Rl=100
V1 a 0 1
R4 a 0 R = (time<1u ? {Rl} : {2*Rl})
.tran 0.1u 2u
.control
run
print i(v1)[1] i(v1)[58] time[58]
.endc
.end
