control_in_subckt
* settle -b: 'sub-block-ran' once, then 'deck-block': a .control block in a .subckt runs once, before the deck's, when the subckt has instances (two here); with none (control_in_subckt_unused.sp) it does not run
.subckt s a
.control
echo sub-block-ran
.endc
r1 a 0 1k
.ends
x1 a s
x2 a s
v1 a 0 dc 1
.control
echo deck-block
op
.endc
.end
