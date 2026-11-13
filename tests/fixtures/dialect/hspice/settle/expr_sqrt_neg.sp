expr_sqrt_neg
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): sqrt(-4) = -2: the sign is kept
vs s 0 1
ve0 e0 0 'sqrt(-4)'
.dc vs 1 1 1
.end
