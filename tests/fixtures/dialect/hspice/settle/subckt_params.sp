subckt_params
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3 (default w=1), i(v2) = -5e-4 (w=2): bare k=v after the ports
.subckt cell a b w=1
r1 a b 'w*1k'
.ends
v1 1 0 1
x1 1 0 cell
v2 2 0 1
x2 2 0 cell w=2
.dc v1 1 1 1
.end
