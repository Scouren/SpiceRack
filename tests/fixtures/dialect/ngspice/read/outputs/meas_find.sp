meas_find
* oracle: ngspice-45 com_measure2.c measure_parse_find, measure_at (linear interpolation at AT), com_measure_when (to= ends the search); runs: f2 = -4.503393e-01, f4 = f1 = -3.170729e-02
.meas tran f1 find v(b) when v(a)=0.5 rise=2
.meas tran f2 find v(b) at=1u
.meas tran f3 find @m1[id] when i(v1)=1m
.meas tran f4 find v(b) when v(a)=0.5 rise=2 to=2u
