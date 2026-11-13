device_ic
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): v(2) = 0.5 at t = 0: a capacitor's ic= is not held without UIC
v1 1 0 1
r1 1 2 1k
c1 2 0 1u ic=0.3
r2 2 0 1k
.tran 1n 2n
.end
