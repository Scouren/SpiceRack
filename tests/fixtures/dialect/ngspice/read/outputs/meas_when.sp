meas_when
* oracle: ngspice-45 com_measure2.c measure_parse_when, com_measure_when (no rise/fall/cross: the first crossing); inpcom.c braces a numparam right-hand side but not v(...); runs: w1 = w4 = 8.33429e-08
.param half=0.5
.meas tran w1 when v(a)=0.5
.meas tran w2 when v(a)=v(b) rise=2 td=1u
.meas tran w3 when v(a)=half fall=last
.meas tran w4 when v(a) = 0.5 cross=1
