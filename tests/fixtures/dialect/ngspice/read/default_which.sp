default_which
* oracle: ngspice-45 run of this deck with `i1 0 n1 1m` and `print v(n1)` added (the test formal_defaults_run_where_the_instance_stands), R53: a head default reads its sibling formals, any other name where the instance stands; v(n1) = 3 (the instance's t=3k, not the definition's 1k)
.param t=1k
.subckt s a b t={t}
r1 a b {t}
.ends
.subckt w a b
.param t=3k
xs a b s
.ends
xw n1 0 w
