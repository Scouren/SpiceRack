icy: an include cycle
* settle (R71, the bound): -b: a segmentation fault ('the monitored command dumped core'), exit 139: ngspice-45 crashes on an include cycle
.include inc_cycle_a.inc
V1 n1 0 1
R1 n1 0 {pa}
.control
op
print i(v1)
.endc
.end
