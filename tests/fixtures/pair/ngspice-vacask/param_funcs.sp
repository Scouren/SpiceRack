param_funcs: parameter functions whose spelling or rule differs between the two (sgn of 0, a power of a negative base, a rounding of a half, abs, sqrt, exp, log) as resistor values of a ladder
.param s0={sgn(0)+2} pw={pwr(-2,2)} rn={nint(2.5)} ab={abs(-3)} sq={sqrt(16)} ex={exp(0)} lg={ln(exp(2))}
v1 in 0 dc 1
r1 in 0 {1k*s0}
r2 in 0 {1k*pw}
r3 in 0 {1k*rn}
r4 in 0 {1k*ab}
r5 in 0 {1k*sq}
r6 in 0 {1k*ex}
r7 in 0 {1k*lg}
.op
.end
