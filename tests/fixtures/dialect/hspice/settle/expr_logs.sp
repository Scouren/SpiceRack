expr_logs
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): log(100) = 4.60517 (natural), log10(100) = 2, exp(1) = 2.71828, sqrt(4) = 2
vs s 0 1
ve0 e0 0 'log(100)'
ve1 e1 0 'log10(100)'
ve2 e2 0 'exp(1)'
ve3 e3 0 'sqrt(4)'
.dc vs 1 1 1
.end
