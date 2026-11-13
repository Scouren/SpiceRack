in_subckt
* oracle: settle control_in_subckt.sp (a block in a .subckt runs once, before the deck's, iff the subckt has an instance; R98: no IR item states it, X0028): its own text, Script
.subckt s a
.control
option reltol=0.01
.endc
r1 a 0 1k
.ends
x1 a s
v1 a 0 dc 1
.control
op
.endc
.end
