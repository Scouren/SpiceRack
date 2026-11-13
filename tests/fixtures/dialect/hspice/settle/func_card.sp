func_card
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): g(2,3) = 23: .func g(x,y) 'x*10+y' is accepted too
.func g(x,y) 'x*10+y'
vs s 0 1
ve0 e0 0 'g(2,3)'
.dc vs 1 1 1
.end
