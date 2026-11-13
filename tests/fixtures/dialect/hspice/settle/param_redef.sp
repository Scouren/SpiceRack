param_redef
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): p = 2: the last .param wins
.param p=1
.param p=2
vs s 0 1
ve0 e0 0 'p'
.dc vs 1 1 1
.end
