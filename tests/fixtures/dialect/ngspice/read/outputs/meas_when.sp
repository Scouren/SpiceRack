meas_when
* oracle: ngspice-45 com_measure2.c measure_parse_when, com_measure_when (no rise/fall/cross: the first crossing after the first interval, 576-579, R30; from= skips points below it as td= does in tran, 478-486; to= stops the search); inpcom.c braces a numparam right-hand side but not v(...); runs: w1 = w4 = 8.33429e-08, from=1u = td=1u = 1.08334e-06, 0.4v = 0.4 (one unit letter of avfc after the number)
.param half=0.5
.meas tran w1 when v(a)=0.5
.meas tran w2 when v(a)=v(b) rise=2 td=1u
.meas tran w3 when v(a)=half fall=last
.meas tran w4 when v(a) = 0.5 cross=1
.meas tran w5 when v(a)=0.5 from=1u
.meas tran w6 when v(a)=0.5 rise=1 to=2u
.meas tran w7 when v(a)=0.4v
