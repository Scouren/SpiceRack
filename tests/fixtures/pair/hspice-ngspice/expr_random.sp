expr_random
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): agauss, gauss, aunif, unif, limit(1,0.5) all 1 (the nominal) outside Monte Carlo
vs s 0 1
ve0 e0 0 'agauss(1,0.1,3)'
ve1 e1 0 'gauss(1,0.1,3)'
ve2 e2 0 'aunif(1,0.5)'
ve3 e3 0 'unif(1,0.5)'
ve4 e4 0 'limit(1,0.5)'
.dc vs 1 1 1
.end
