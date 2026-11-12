wrapped_param
* oracle: settle/wrap_param_{1,2,3,4}.sp ({a}+{b} and 'a'+'b': '; sign expected.', fatal; 'a+{b}' and {a+'b'}: 'Syntax error: letter [{]', 'Formula() error.', fatal)
.param a=1 b=2
.param p1={a}+{b}
.param p2='a'+'b'
.param p3='a+{b}'
.param p4={a+'b'}
