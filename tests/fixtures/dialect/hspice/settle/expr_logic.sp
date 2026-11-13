expr_logic
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 1<2 = 1, 2!=3 = 1, 1&&0 = 0, 1||0 = 1, 1?0?4:8:16 = 8, sgn(-3) = -1, sign(-3,2) = 0 (the old notes' manual lead says 3: ESPice only), abs(-3) = 3, min = 1, max = 2
vs s 0 1
ve0 e0 0 '1<2'
ve1 e1 0 '2!=3'
ve2 e2 0 '1&&0'
ve3 e3 0 '1||0'
ve4 e4 0 '1?0?4:8:16'
ve5 e5 0 'sgn(-3)'
ve6 e6 0 'sign(-3,2)'
ve7 e7 0 'abs(-3)'
ve8 e8 0 'min(1,2)'
ve9 e9 0 'max(1,2)'
.dc vs 1 1 1
.end
