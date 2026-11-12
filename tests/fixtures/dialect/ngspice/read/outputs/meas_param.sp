meas_param
* oracle: ngspice-45 measure.c do_measure (every other measure first, its result a numparam parameter; then param= by nupa_eval); runs: p1 = 2*tr, p3 reads the later measure 'later'; a param measure reading a later param measure fails ('Undefined parameter', run p6 failed) unless a .param of the name exists, which it reads (run p4 = 7: not typed)
.param k=2
.meas tran tr when v(a)=0.5
.meas tran p1 param='tr*k'
.meas tran p2 param={later+1}
.meas tran later max v(a)
.meas tran p3 param='p4*2'
.meas tran p5 param='k*p6'
.meas tran p4 param='3'
.meas tran p6 param='1'
.param p6=5
