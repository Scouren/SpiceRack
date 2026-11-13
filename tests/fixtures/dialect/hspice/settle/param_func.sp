param_func
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): f(2,3) = 23: .param f(x,y)='x*10+y'
.param f(x,y)='x*10+y'
vs s 0 1
ve0 e0 0 'f(2,3)'
.dc vs 1 1 1
.end
