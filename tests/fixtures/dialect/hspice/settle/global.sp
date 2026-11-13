global
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(vdd) = -1e-3: .global vdd
.global vdd
.subckt cell a
r1 a vdd 1k
.ends
vdd vdd 0 1
x1 0 cell
.dc vdd 1 1 1
.end
