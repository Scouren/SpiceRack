funcs_a11
* oracle: ngspice-45 ptfuncs.c PTustep (u is 0.5 at 0), PTpwr (pwr is sgn(x)|x|^y), xpressn.c mathfunction (numparam sqr is x*x); runs B u(0) = 0.5, B pwr(-3,2) = -9, {sqr(3)} = 9; IR-KERNEL A11
.param a={u(temper-27)} b={pwr(temper-30,3)} c={sqr(3)}
