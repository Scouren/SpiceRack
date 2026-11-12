wrapped
* oracle: settle/wrap_analysis_{1,2,3,4}.sp ({a}+{b} and 'a'+'b': 'Bad syntax!'; 'a+{b}' and {a+'b'}: 'Syntax error: letter [{]', fatal)
.param a=1 b=2
.dc v1 0 {a}+{b} 1
.dc v1 0 'a'+'b' 1
.dc v1 0 'a+{b}' 1
.dc v1 0 {a+'b'} 1
