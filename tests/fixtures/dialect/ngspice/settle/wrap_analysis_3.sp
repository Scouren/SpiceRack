wanalysis3: an analysis field (stop 3 gives 4 points, stop 1 gives 2): 'a+{b}'
* settle (audit row 4): -b: 'Netlist line no. 6:', 'Syntax error: letter [{]', 'Expression err: a+{b}} 1', 'Cannot compute substitute', fatal (invalid in source)
.param a=1 b=2
V1 n 0 1
R1 n 0 1
.dc v1 0 'a+{b}' 1
.control
set numdgt=17
run
print length(v(n))
.endc
.end
