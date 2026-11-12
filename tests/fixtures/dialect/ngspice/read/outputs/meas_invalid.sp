meas_invalid
* oracle: ngspice-45 get_measure2 (deriv, err: 'currently not supported'; integral: 'no such function'), do_measure (op: 'unrecognized analysis type'; three words: 'incomplete measurement statement'), measure_parse_when ('Expressions like v(b)*2 are not supported'); runs: find v(a,b) 'no such vector', td=<measure> 'Undefined parameter' (fatal)
.meas tran d1 deriv v(a) at=1u
.meas tran d2 find v(a,b) at=1u
.meas tran d3 integral v(a)
.meas op d4 find v(a) at=1
.meas tran w when v(a)=0.5
.meas tran d5 when v(a)=0.5 td=w
.meas tran d6 err v(a)
.meas tran d7 when v(a)=v(b)*2
.meas tran d8
