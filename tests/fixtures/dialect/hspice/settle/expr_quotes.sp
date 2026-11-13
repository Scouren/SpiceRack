expr_quotes
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): '2*3' = "2*3" = 6: both quotes delimit an expression
.param a=2
vs s 0 1
ve0 e0 0 '2*3'
ve1 e1 0 "2*3"
.dc vs 1 1 1
.end
