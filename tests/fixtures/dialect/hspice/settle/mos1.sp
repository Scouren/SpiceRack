mos1
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(vd) = -1.69600003e-04 (ngspice twin: the same)
.option tnom=27
.temp 27
vd d 0 3
vg g 0 1.5
m1 d g 0 0 nm w=10u l=2u
.model nm nmos level=1 vto=0.7 kp=1e-4 lambda=0.02
.dc vd 3 3 1
.end
