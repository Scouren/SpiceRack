isf: a subcircuit that instantiates itself
* settle (X0024): -b: 'Error: unknown subckt: x1.xw.xw… (22 levels) n1 0 w', 'in line no. 5', 'Simulation interrupted due to error!', exit 1: ngspice rejects a subcircuit that instantiates itself
.subckt w a b
R1 a b 1k
xw a 0 w
.ends
V1 n1 0 1
X1 n1 0 w
.control
op
print i(v1)
.endc
.end
