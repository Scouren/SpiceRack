param_string
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): t2 = 8: .param t2="a^3" is an expression, not a string
.param a=2
.param t2="a^3"
vs s 0 1
ve0 e0 0 't2'
.dc vs 1 1 1
.end
