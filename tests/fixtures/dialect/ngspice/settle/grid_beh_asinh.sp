grid_beh_asinh
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -1000.0
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
va14 a14 0 dc 1000.0
.nodeset v(a0)=-1000.0 v(a1)=-3 v(a2)=-2.5 v(a3)=-1.5 v(a4)=-1 v(a5)=-0.5 v(a6)=-1e-07 v(a7)=0 v(a8)=1e-07 v(a9)=0.5 v(a10)=1 v(a11)=1.5 v(a12)=2.5 v(a13)=3 v(a14)=1000.0
bd186 d186 0 v = asinh(v(a1))
bd187 d187 0 v = asinh(v(a2))
bd188 d188 0 v = asinh(v(a3))
bd189 d189 0 v = asinh(v(a4))
bd190 d190 0 v = asinh(v(a5))
bd191 d191 0 v = asinh(v(a6))
bd192 d192 0 v = asinh(v(a7))
bd193 d193 0 v = asinh(v(a8))
bd194 d194 0 v = asinh(v(a9))
bd195 d195 0 v = asinh(v(a10))
bd196 d196 0 v = asinh(v(a11))
bd197 d197 0 v = asinh(v(a12))
bd198 d198 0 v = asinh(v(a13))
bd199 d199 0 v = asinh(v(a0))
bd200 d200 0 v = asinh(v(a14))
.op
.end
