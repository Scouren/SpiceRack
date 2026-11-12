subckt_param: a subcircuit instance with a parameter: a divider whose lower leg is the instance's rlo
.subckt div top mid rlo=1k
r1 top mid 1k
r2 mid 0 {rlo}
.ends div
v1 in 0 dc 3
x1 in out div rlo=2k
.op
.end
