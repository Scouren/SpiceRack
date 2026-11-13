formal_scope
* oracle: settle formal_scope.sp (a formal binds in its own subckt; the top reads the top w; F34: the reader bound every subckt's formal w to the last one, b's)
.param w=9
.subckt a 1 2 w=1
.param x={w*2}
r1 1 2 {x}
.ends
.subckt b 1 2 w=1
.param y={w*3}
r1 1 2 {y}
.ends
.param t={w*5}
x1 n 0 a
x2 m 0 b w=4
