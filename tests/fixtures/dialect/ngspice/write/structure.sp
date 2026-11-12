structure
* a comment
.global vdd vss
.include "a.sp"
.lib "c.lib" tt
.param w=2
.func sq(x) {x * x}
.subckt cell a b params: w=1 l={w * 2}
.param k={l + 1}
.ends
.if (w == 1)
* one
.elseif (w > 1)
.else
* other
.endif
.end
