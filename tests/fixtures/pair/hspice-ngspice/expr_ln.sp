expr_ln
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): ln(100) = 4.60517: ln is accepted as log
vs s 0 1
ve0 e0 0 'ln(100)'
.dc vs 1 1 1
.end
