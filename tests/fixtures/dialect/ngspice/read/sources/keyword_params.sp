keyword_params
* oracle: ngspice-45 examples numparam/param-example.cir, vdmos/100W.sp, xspice/delay/analog-delay1.cir (parameters named like source keywords, quoted or braced: numparam substitutes them first); runs give the values printed; the writer braces them (bare `phase`, `v`, `freq` would read as keywords)
.param amplitude=1 phase=0 dcvoltage=0 V=1.44 freq=1k
V1 a 0 DC 'dcvoltage' AC 'amplitude' 'phase'
V2 b 0 ac 0 dc 0 SINE(0 {V} 1K)
V3 c 0 dc 0 sin(0 1 'freq')
.control
op
print v(a) v(b) v(c)
tran 10u 1m
print v(b)[30] v(c)[30]
.endc
.end
