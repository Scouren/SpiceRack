instance_ports
* oracle: settle/instance_ports_short.sp ('Too few parameters for subcircuit type'), settle/instance_ports_long.sp ('Too many parameters'; its neighbour runs): R61, A36; a master no file defines is not checked
.subckt sub a b
.ends
X1 n1 sub
X2 n1 n2 n3 sub
X3 n1 n2 sub
X4 n1 n2 n3 nowhere
X5 n1 later
.subckt later a b params: w=1
.ends
