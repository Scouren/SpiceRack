wrapped
* oracle: settle/wrap_model_{1,2,3,4}.sp ({a}+{b} and 'a'+'b': 'unrecognized parameter (+) - ignored', rsh = 1: the deck runs on the first wrapper alone, a form with no rule here; 'a+{b}' and {a+'b'}: 'Syntax error: letter [{]', fatal)
.param a=1 b=2
.model rm1 r rsh={a}+{b}
.model rm2 r rsh='a'+'b'
.model rm3 r rsh='a+{b}'
.model rm4 r rsh={a+'b'}
