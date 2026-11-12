om: an OSDI instance's m=
* settle (census W1100 1a): resva compiled by openvaf from res.va (I = V/r); -b: i(v1) = -3.00000e-03 (N1 with m=2 and N2 without: 2 mA + 1 mA); the neighbour with no m= on N1 gives -2.00000e-03: on ngspice's N card m= is the multiplicity
*# pre_osdi res.osdi
V1 a 0 1
N1 a 0 rmod m=2
N2 a 0 rmod
.model rmod resva r=1k
.control
op
print i(v1)
.endc
.end
