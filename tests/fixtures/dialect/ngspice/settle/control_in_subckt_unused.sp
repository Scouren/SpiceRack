control_in_subckt_unused
* settle -b: no 'sub-block-ran': the subckt has no instance and its .control block does not run
.subckt s a
.control
echo sub-block-ran
.endc
r1 a 0 1k
.ends
v1 a 0 dc 1
r1 a 0 1k
.op
.end
