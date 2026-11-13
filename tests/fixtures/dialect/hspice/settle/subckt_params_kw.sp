subckt_params_kw
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(v1) = -1e-3: 'params:' is accepted too
.subckt cell a b params: w=1
r1 a b 'w*1k'
.ends
v1 1 0 1
x1 1 0 cell
.dc v1 1 1 1
.end
