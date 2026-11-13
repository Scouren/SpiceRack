measure_forms
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): tcross = 5e-7; tdelta = 6e-7 (targ 8e-7, trig 2e-7): 'when v(1)=0.5' and 'trig v(1) val=0.2 rise=1 targ …'
v1 1 0 pulse(0 1 0 1u 1u 1 2)
r1 1 0 1k
.tran 0.1u 1u
.meas tran tcross when v(1)=0.5
.meas tran tdelta trig v(1) val=0.2 rise=1 targ v(1) val=0.8 rise=1
.end
