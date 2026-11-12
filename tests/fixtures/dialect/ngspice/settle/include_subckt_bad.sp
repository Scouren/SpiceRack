isb: an X card before the .include that defines its subcircuit, one node too many
* settle (the later-file subcircuit binding; R61, A36): -b: 'Too many parameters for subcircuit type "sub" (instance: xx1)', 'Simulation interrupted due to error!', exit 1: the port count of a .subckt in a later included file is checked as one in the deck's own text
V1 n1 0 1
X1 n1 n2 sub
.include include_subckt.inc
.control
op
print i(v1)
.endc
.end
