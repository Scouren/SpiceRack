inst_cycle_inc
* settle -b: "Error: unknown subckt: x1.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa.xb.xa n a", "incomplete or empty netlist", exit 1: a calls b (inst_cycle_inc.inc), b calls a
.include "inst_cycle_inc.inc"
.subckt a p
xb p b
.ends
x1 n a
v1 n 0 1
.op
.end
