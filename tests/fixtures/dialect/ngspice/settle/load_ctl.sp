l_ctl: OSDI load form ctl
* settle (S3-CONTROL L1): resva compiled by openvaf from res.va (I = V/r); results: ctl v(b)=0.5, hash v(b)=0.5, dot: model unknown (no .pre_osdi card in ngspice-45), late (osdi after parse): model unknown
V1 a 0 1
R1 a b 1k
N1 b 0 rmod
.model rmod resva r=1k
.control
pre_osdi res.osdi
.endc
.control
op
print v(b)
.endc
.end
