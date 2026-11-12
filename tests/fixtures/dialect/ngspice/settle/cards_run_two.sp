cr2: two card analyses (a tran, then an op) and one run
* settle (R85): -b: the op runs first ('No. of Data Rows : 1'), then 'Error: no data saved for Transient analysis; analysis not run', 'doAnalyses: not found', 'run simulation(s) aborted', yet a tran1 plot of 78 rows exists; exit 1: two cards and one run do not run in card order and the run fails
V1 a 0 dc 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
R2 b 0 1k
C1 b 0 1n
.tran 0.1u 1u
.op
.control
run
setplot
print v(b)
.endc
.end
