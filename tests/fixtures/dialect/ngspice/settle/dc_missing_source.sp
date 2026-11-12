dms: a DC sweep of a source no statement defines
* settle (R70, A43): -b: 'Fatal error: DC Transfer Function: Voltage source, current source, or resistor named "vnope" is not in the circuit', 'doAnalyses: no such device', 'run simulation(s) aborted'; the script's dc rnope the same; exit 1: the analysis does not run
V1 n 0 1
R1 n 0 1k
.dc vnope 0 1 1
.control
run
print v(n)
dc rnope 1k 2k 1k
print v(n)
.endc
.end
