instance_cycle
* oracle: settle/instance_self.sp, settle/instance_cycle.sp ('unknown subckt: x1.xw.xw…', exit 1; X0024); the neighbour leaf in top is a hierarchy
.subckt w a b
R1 a b 1k
xw a 0 w
.ends
.subckt p a b
xq a b q
.ends
.subckt q a b
xp a b p
.ends
.subckt leaf a
R2 a 0 1k
.ends
.subckt top a
xl a leaf
.ends
X1 n1 0 w
X2 n1 0 p
X3 n1 top
