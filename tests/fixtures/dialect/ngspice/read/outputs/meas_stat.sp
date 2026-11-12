meas_stat
* oracle: ngspice-45 com_measure2.c measure_minMaxAvg, measure_rms_integral (from=, to= bound the window; none given: the whole analysis); runs: av = -1.395530e-04, rm = 7.07109e-01
.meas tran av avg v(a) from=0.5u to=1.5u
.meas tran mx max v(b)
.meas tran mn min v(b) to=2u
.meas tran p pp v(b) from=1u
.meas tran r rms v(a)
.meas tran ig integ v(a) from=0 to=1u
