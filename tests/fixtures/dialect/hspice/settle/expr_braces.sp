expr_braces
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): exit 1 (UnresolvedParameter): r1 s 0 {2*1k}, braces are no expression
vs s 0 1
r1 s 0 {2*1k}
.dc vs 1 1 1
.end
