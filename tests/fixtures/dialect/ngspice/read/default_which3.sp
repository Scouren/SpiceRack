default_which3
* oracle: ngspice-45 run of this deck with `i1 0 n1 1m` and `print v(n1)` added (the test formal_defaults_run_where_the_instance_stands), R53: a head default reads its sibling formals, any other name where the instance stands; v(n1) = 6 (s defined and instantiated in w; l reads its sibling t, 3k)
.param t=1k
.subckt w a b
.param t=3k
.subckt s a b t={t} l={t*2}
r1 a b {l}
.ends
xs a b s
.ends
xw n1 0 w
