macro
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -5e-4: .macro/.eom are .subckt/.ends
.macro cell a b
r1 a b 2k
.eom
v1 1 0 1
x1 1 0 cell
.dc v1 1 1 1
.end
