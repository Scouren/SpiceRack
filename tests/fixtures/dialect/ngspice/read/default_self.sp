default_self
* oracle: ngspice-45 runs (scratchpad runs/selfdef): outer.cir, selfplus.cir (a head default reading its own name reads the enclosing t: i = -5e-4, -2.5e-4); later.cir, earlier.cir (another formal is the formal); mutual.cir (a cycle stops ngspice)
.param t=2k
.subckt s a b t={t*2} w={l} l=1k
r1 a b {t}
.ends
.subckt m a w={l} l={w}
.ends
