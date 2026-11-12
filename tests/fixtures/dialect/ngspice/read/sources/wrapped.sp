wrapped
* oracle: settle/wrap_source_{1,2,3,4}.sp ({a}+{b} and 'a'+'b': 'unknown parameter (+)'; 'a+{b}' and {a+'b'}: v(n) = 3)
.param a=1 b=2
V1 n 0 {a}+{b}
V2 n 0 'a'+'b'
V3 n 0 'a+{b}'
V4 n 0 {a+'b'}
