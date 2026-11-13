wrapped
* oracle: settle/wrap_element_{1,2,3,4}.sp ({a}+{b} and 'a'+'b': 'unknown parameter (+)', each wrapper its own expression; 'a+{b}' and {a+'b'}: R = 3, a wrapper inside the other is a parenthesis)
.param a=1 b=2
R1 n 0 {a}+{b}
R2 n 0 'a'+'b'
R3 n 0 'a+{b}'
R4 n 0 {a+'b'}
