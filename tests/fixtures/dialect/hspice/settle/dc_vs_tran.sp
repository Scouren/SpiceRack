dc_vs_tran
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): v(1) = 1 at t = 0 though dc 5: the waveform's t = 0 value overrides DC in a transient
v1 1 0 dc 5 pulse(1 2 1u 1n 1n 1u 2u)
r1 1 0 1k
.tran 0.1u 0.5u
.end
