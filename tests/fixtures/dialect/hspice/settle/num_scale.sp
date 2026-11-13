num_scale
* settle (ESPice 0.3.0, --tokenizer=hspice; ESPice is not HSPICE): 1M = 1e-3, 1MEG = 1X = 1e6, 1MIL = 2.54e-5, 1A = 1e-18, 1G, 1T, 1F, 10pf = 1e-11, 1d3 = 1000, 2.5D-3 = 2.5e-3, 1k5 = 1000, 1kHz = 1000
vs s 0 1
ve0 e0 0 1M
ve1 e1 0 1MEG
ve2 e2 0 1X
ve3 e3 0 1MIL
ve4 e4 0 1A
ve5 e5 0 1G
ve6 e6 0 1T
ve7 e7 0 1F
ve8 e8 0 10pf
ve9 e9 0 1d3
ve10 e10 0 2.5D-3
ve11 e11 0 1k5
ve12 e12 0 1kHz
.dc vs 1 1 1
.end
