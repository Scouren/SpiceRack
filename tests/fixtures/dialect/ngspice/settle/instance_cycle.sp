icy2: two subcircuits that instantiate each other
* settle (X0024): -b: 'Error: unknown subckt: x1.xq.xp.xq… n1 0 p', 'in line no. 9', 'Simulation interrupted due to error!', exit 1: a cycle through two subcircuits too
.subckt p a b
R1 a b 1k
xq a b q
.ends
.subckt q a b
R2 a b 1k
xp a b p
.ends
V1 n1 0 1
X1 n1 0 p
.control
op
print i(v1)
.endc
.end
