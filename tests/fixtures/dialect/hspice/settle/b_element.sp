b_element
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1, 'unsupported HSPICE element': B is no behavioural source
v1 1 0 1
b1 2 0 v='v(1)*2'
r2 2 0 1k
.dc v1 1 1 1
.end
