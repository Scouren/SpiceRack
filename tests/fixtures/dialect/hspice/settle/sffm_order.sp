sffm_order
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): v(1) = 1.000327 at 20 ns: SFFM(VO VA FC MDI FS), SPICE order
v1 1 0 sffm(1 2 1k 3 100)
r1 1 0 1k
.tran 0.05m 0.2m
.end
