meas_trig_targ
* oracle: ngspice-45 com_measure2.c measure_parse_trigtarg, measure_parse_stdParams (val is 0 when not given, the last of rise/fall/cross wins, LAST); get_measure2 (a FROM of one clause is the other's when that one has none and it is not 0); runs: tr = 1.156372e-07, t3 equal with and without cross=2 and val=0, t4 = td=1u in both clauses = 4.566531e-07
.meas tran tr trig v(a) val=0.2 rise=1 targ v(a) val=0.8 rise=1
.meas tran t2 trig at=1u targ v(b) val=0.5 fall=last td=2u
.measure TRAN t3 TRIG v(a) VAL=0.1 CROSS=2 RISE=3 TARG i(v1) cross=last
.meas tran t4 trig v(a) val=0.2 rise=1 from=1u targ v(b) val=0.5 fall=1
