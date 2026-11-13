grid_beh_exp
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -250
va1 a1 0 dc -50
va2 a2 0 dc -30
va3 a3 0 dc -3
va4 a4 0 dc -2.5
va5 a5 0 dc -1.5
va6 a6 0 dc -1
va7 a7 0 dc -0.5
va8 a8 0 dc -1e-07
va9 a9 0 dc 0
va10 a10 0 dc 1e-07
va11 a11 0 dc 0.5
va12 a12 0 dc 1
va13 a13 0 dc 1.5
va14 a14 0 dc 2.5
va15 a15 0 dc 3
va16 a16 0 dc 30
va17 a17 0 dc 50
va18 a18 0 dc 250
.nodeset v(a0)=-250 v(a1)=-50 v(a2)=-30 v(a3)=-3 v(a4)=-2.5 v(a5)=-1.5 v(a6)=-1 v(a7)=-0.5 v(a8)=-1e-07 v(a9)=0 v(a10)=1e-07 v(a11)=0.5 v(a12)=1 v(a13)=1.5 v(a14)=2.5 v(a15)=3 v(a16)=30 v(a17)=50 v(a18)=250
bd23 d23 0 v = exp(v(a3))
bd24 d24 0 v = exp(v(a4))
bd25 d25 0 v = exp(v(a5))
bd26 d26 0 v = exp(v(a6))
bd27 d27 0 v = exp(v(a7))
bd28 d28 0 v = exp(v(a8))
bd29 d29 0 v = exp(v(a9))
bd30 d30 0 v = exp(v(a10))
bd31 d31 0 v = exp(v(a11))
bd32 d32 0 v = exp(v(a12))
bd33 d33 0 v = exp(v(a13))
bd34 d34 0 v = exp(v(a14))
bd35 d35 0 v = exp(v(a15))
bd36 d36 0 v = exp(v(a2))
bd37 d37 0 v = exp(v(a16))
bd38 d38 0 v = exp(v(a1))
bd39 d39 0 v = exp(v(a17))
bd40 d40 0 v = exp(v(a0))
bd41 d41 0 v = exp(v(a18))
.op
.end
