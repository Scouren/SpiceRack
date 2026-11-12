temp_diode: a diode's operating point at 75 C (the temperature option), nominal 27 C
v1 in 0 dc 2
r1 in a 1k
d1 a 0 dmod
.model dmod d is=1e-14 n=1.05 rs=10
.options temp=75
.op
.end
