diode: a diode (SPICE level 1 model) behind a resistor, its forward curve swept by the source
v1 in 0 dc 0
r1 in a 1k
d1 a 0 dmod
.model dmod d is=1e-14 n=1.05 rs=10
.dc v1 0 2 0.1
.end
