default_which4
* oracle: ngspice-45 run of this deck with `i1 0 n1 1m` and `print v(n1)` added (the test formal_defaults_run_where_the_instance_stands), R53: a .func call in a head default is the definition scope's (lexical), unlike a parameter read; v(n1) = 2 (the top f, x*2, not the wrapper's x*8)
.func f(x) {x*2}
.subckt s a b t={f(1k)}
r1 a b {t}
.ends
.subckt w a b
.func f(x) {x*8}
xs a b s
.ends
xw n1 0 w
