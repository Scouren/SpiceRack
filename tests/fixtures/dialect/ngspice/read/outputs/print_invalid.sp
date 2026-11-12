print_invalid
* oracle: run: .print ac vi(x1.n) saves x1.n#branch (gettoks) and the analysis does not run; .print tran with no output warns 'no nodes given' and prints nothing; a column expression and .print op are not typed
.print ac vi(a)
.print tran
.print tran v(a) -v(b)
.print op v(a)
