expr_power
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 2**3**2 = 64 (left-associative), -2**2 = -4, (-2)**3 = -8 (sign kept), (-2.5)**2.1 = 6.25 (the exponent truncated to 2), pow(-2,3) = -8, pwr(-2,3) = 8, 2^3 = 8 ('^' is a power), 2^3^2 = 64
vs s 0 1
ve0 e0 0 '2**3**2'
ve1 e1 0 '-2**2'
ve2 e2 0 '(-2)**3'
ve3 e3 0 '(-2.5)**2.1'
ve4 e4 0 'pow(-2,3)'
ve5 e5 0 'pwr(-2,3)'
ve6 e6 0 '2^3'
ve7 e7 0 '2^3^2'
.dc vs 1 1 1
.end
