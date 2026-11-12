ic_subckt
* oracle: run icsub.cir (a .ic in a .subckt body holds each instance's own node: v(x1.x) = v(x2.x) = 0.5, the top x untouched)
.subckt s1 in
.ic v(x)=0.5
.ends
