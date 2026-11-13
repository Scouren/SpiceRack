timed_op
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): a transient plot and two operating-point plots at time=1e-9 and 5e-9: '.op 1n 5n' is a timed report
v1 1 0 pulse(0 1 0 1n 1n 1 2)
r1 1 2 1k
c1 2 0 1p
.tran 0.1n 5n
.op 1n 5n
.end
