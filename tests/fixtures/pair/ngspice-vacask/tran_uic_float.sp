tran_uic_float: a floating capacitor with ic=2 under uic (from passes' settle_ng3): at t = 0 VACASK stores the unsolved start (a = 2, b = 0), ngspice its first consistent solve (a = 0.667, b = -1.333); from the first step on they agree. Proves the comparison's uic rule (cmp::without_start) and device_ic_to_tran_ic
r1 a 0 1k
r2 b 0 2k
c1 a b 1u ic=2
.tran 10u 3m uic
.end
