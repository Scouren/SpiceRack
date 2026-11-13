grid_beh_cosh
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -30
va1 a1 0 dc -3
va2 a2 0 dc -2.5
va3 a3 0 dc -1.5
va4 a4 0 dc -1
va5 a5 0 dc -0.5
va6 a6 0 dc -1e-07
va7 a7 0 dc 0
va8 a8 0 dc 1e-07
va9 a9 0 dc 0.5
va10 a10 0 dc 1
va11 a11 0 dc 1.5
va12 a12 0 dc 2.5
va13 a13 0 dc 3
va14 a14 0 dc 30
.nodeset v(a0)=-30 v(a1)=-3 v(a2)=-2.5 v(a3)=-1.5 v(a4)=-1 v(a5)=-0.5 v(a6)=-1e-07 v(a7)=0 v(a8)=1e-07 v(a9)=0.5 v(a10)=1 v(a11)=1.5 v(a12)=2.5 v(a13)=3 v(a14)=30
bd156 d156 0 v = cosh(v(a1))
bd157 d157 0 v = cosh(v(a2))
bd158 d158 0 v = cosh(v(a3))
bd159 d159 0 v = cosh(v(a4))
bd160 d160 0 v = cosh(v(a5))
bd161 d161 0 v = cosh(v(a6))
bd162 d162 0 v = cosh(v(a7))
bd163 d163 0 v = cosh(v(a8))
bd164 d164 0 v = cosh(v(a9))
bd165 d165 0 v = cosh(v(a10))
bd166 d166 0 v = cosh(v(a11))
bd167 d167 0 v = cosh(v(a12))
bd168 d168 0 v = cosh(v(a13))
bd169 d169 0 v = cosh(v(a0))
bd170 d170 0 v = cosh(v(a14))
.op
.end
