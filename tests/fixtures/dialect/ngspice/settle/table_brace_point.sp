table_brace_point
* settle -b: 'v(o) = 1.000000e+00' (({p}, 2) is (1, 2): numparam substitutes the brace before the table becomes a pwl); with ({p*2}, 2) 'v(o) = 5.000000e-01'
.param p=1
vi i 0 dc 0.5
E1 o 0 TABLE {v(i)} = (0, 0) ({p}, 2)
R1 o 0 1k
.control
op
print v(o)
.endc
.end
