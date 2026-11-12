meas_stat
* oracle: ngspice-45 com_measure2.c measure_minMaxAvg, measure_rms_integral (from=, to= bound the window; none given: the whole analysis); AT_MIN_AT, AT_MAX_AT give the position; dc: a window given high to low is swapped (measure_parse_stdParams, make m_from always less than m_to), and to=0 is a bound there; runs: av = -1.395530e-04, rm = 7.07109e-01, max_at 3.328e-07
.meas tran av avg v(a) from=0.5u to=1.5u
.meas tran mx max v(b)
.meas tran mn min v(b) to=2u
.meas tran p pp v(b) from=1u
.meas tran r rms v(a)
.meas tran ig integ v(a) from=0 to=1u
.meas tran mxa max_at v(b) from=1u
.meas tran mna min_at v(b)
.meas dc s1 avg v(a) from=0.8 to=0.2
.meas dc s2 max v(a) to=0
