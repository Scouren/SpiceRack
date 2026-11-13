expr_round
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): nint(2.5) = 2, nint(-2.5) = -2, nint(-0.5) = 0 (half to even; recorded HSPICE: 3, -3, -1: F68), int(-2.5) = -2, floor(-2.5) = -3, ceil(-2.5) = -2
vs s 0 1
ve0 e0 0 'nint(2.5)'
ve1 e1 0 'nint(-2.5)'
ve2 e2 0 'nint(-0.5)'
ve3 e3 0 'int(-2.5)'
ve4 e4 0 'floor(-2.5)'
ve5 e5 0 'ceil(-2.5)'
.dc vs 1 1 1
.end
