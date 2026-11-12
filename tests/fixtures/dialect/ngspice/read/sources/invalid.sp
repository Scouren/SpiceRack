invalid
* oracle: runs on ngspice-45: each card stops the run ("unknown parameter", "parameter value out of range", "no such parameter", "Undefined parameter"); `external` is SHARED_MODULE only (vsrc.c)
V1 a 0 1 2
V2 b 0 dc 1 m 2
I3 c 0 pwl(0 0 1u 1 2u 0) r=0
V4 d 0 pulse(1)
V5 e 0 pwl(0 0 1u 1 2u)
V6 f 0 pwl(0 0 1u 1 2u 0) r=0.5u
V7 g 0 dc 0 external
V8 h 0 {time*1}
V9 i 0 distof1 distof2 0.1
V10 j 0 dc 1 ac 1 2 3
V11
.end
