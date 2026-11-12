meas_dc_window
* oracle: ngspice-45 com_measure2.c measure_parse_stdParams (dc: make m_from always less than m_to, a swap by value after substitution); the IR has no window in either order (R18), so a dc window with an expression bound is Unread under its own code
.param k=0.8
.meas dc a1 avg v(a) from={k} to=0.2
.meas dc a2 max v(a) from=0.1 to=k
