expr_power
* oracle: INV-semantics 2.02-2.04 (ngspice ** and ^ are |x|^y, left-associative, unary minus looser), runs {2**3**2} = 64, {-2**2} = -4, {2*-2**2} = 8, {pow(0-2,3)} = -8
.param a={2**3**2} b={-2**2} c={2*-2**2} d={pow(0-2,3)} e={pwr(2,3)}
