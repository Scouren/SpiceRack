diode_op: the operating point of a diode (SPICE level 1 model, area 2) behind a resistor: the model family converts without a sweep
v1 in 0 dc 2
r1 in a 1k
d1 a 0 dmod area=2
.model dmod d is=1e-14 n=1.05 rs=10
.op
.end
