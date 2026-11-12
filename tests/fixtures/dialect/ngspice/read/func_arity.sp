func_arity
* oracle: ngspice-45 runs (scratchpad runs/arity: f(1, 2), f() and a B source f(1,2) on a one-formal .func stop ngspice: "parameter mismatch for function call")
.func f(x) {x*2}
.param p={f(1, 2)}
.param q={f(3)} r={f()}
.param s={g(1, 2)}
