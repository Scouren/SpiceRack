unresolved_param: a parameter read no definition binds, in a complete deck (R63, A38: a strict read refuses it; ngspice rejects it: "Undefined parameter")
v1 in 0 dc 1
r1 in 0 {nosuch}
.op
.end
