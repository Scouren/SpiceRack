bistable_dc: v(out) = 2*tanh(10*(v(out)+v(in))), stable at about -2 and +2 for |v(in)| < 1.73; a continued .dc from -2 V holds -2 V up to 1 V (ngspice-45: -2.00000e+00 at every point)
vin in 0 dc 0
b1 0 out i=2m*tanh(10*(v(out)+v(in)))
r1 out 0 1k
.dc vin -2 1 0.5
.control
run
print v(out)
.endc
.end
