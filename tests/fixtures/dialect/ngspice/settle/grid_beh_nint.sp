grid_beh_nint
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
bd417 d417 0 v = nint(v(a1))
bd418 d418 0 v = nint(v(a2))
bd419 d419 0 v = nint(v(a3))
bd420 d420 0 v = nint(v(a4))
bd421 d421 0 v = nint(v(a5))
bd422 d422 0 v = nint(v(a6))
bd423 d423 0 v = nint(v(a7))
bd424 d424 0 v = nint(v(a8))
bd425 d425 0 v = nint(v(a9))
bd426 d426 0 v = nint(v(a10))
bd427 d427 0 v = nint(v(a11))
bd428 d428 0 v = nint(v(a12))
bd429 d429 0 v = nint(v(a13))
bd430 d430 0 v = nint(v(a0))
bd431 d431 0 v = nint(v(a14))
bd432 d432 0 v = nint(v(a2))
bd433 d433 0 v = nint(v(a12))
bd434 d434 0 v = nint(v(a9))
.op
.end
