f13b: the rcl shape: save all and a device output the isource lacks
* settle: -b: 'Warning from checkvalid: vector @i1[i] is not available or has zero length.', 'Error during 'write': no writable vector found.' (F13, demo/spice/rcl.sim)
V1 a 0 dc 1
R1 a b 1k
I1 b 0 dc 1m
.control
save all @i1[i]
tran 0.1n 5n 0 0.1n
write f13b.raw
.endc
.end
