meas_param
* oracle: ngspice-45 measure.c do_measure (every other measure first, its result a numparam parameter; then param= by nupa_eval); runs: p1 = 2*tr, p3 reads the later measure 'later'
.param k=2
.meas tran tr when v(a)=0.5
.meas tran p1 param='tr*k'
.meas tran p2 param={later+1}
.meas tran later max v(a)
