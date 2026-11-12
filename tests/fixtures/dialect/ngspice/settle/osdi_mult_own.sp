oom: a module that declares its own parameter m
* settle (census W1100 1a): resm compiled by openvaf from resm.va (I = m*V/r, parameter m); -b: 'Error on line 5 or its substitute:', 'unknown parameter (m)', 'Simulation interrupted due to error!': m= on an N is never the module's own parameter
*# pre_osdi resm.osdi
V1 a 0 1
N1 a 0 rmod m=2
.model rmod resm r=1k
.control
op
print i(v1)
.endc
.end
