temp_card: a diode's operating point under the .temp card at 75 C (the deck-level temperature, not an option)
v1 in 0 dc 2
r1 in a 1k
d1 a 0 dmod
.model dmod d is=1e-14 n=1.05 rs=10
.temp 75
.op
.end
