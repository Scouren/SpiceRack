pulse_zero_rise
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 0 until 1 us, then a ramp to 1 at 1.1 us: a given zero rise is TSTEP (0.1 us)
v1 1 0 pulse(0 1 1u 0 0 1u 4u)
r1 1 0 1k
.tran 0.1u 3u
.end
