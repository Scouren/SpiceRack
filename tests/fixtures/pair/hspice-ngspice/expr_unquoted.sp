expr_unquoted
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): i(vs) = -2.5e-4: r=b*4k/3, an unquoted expression led by a letter, is 4k
vs s 0 1
.param b=3
r1 s 0 r=b*4k/3
.dc vs 1 1 1
.end
