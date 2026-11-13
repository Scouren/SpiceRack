mvi: a measure of an ac imaginary part, vi()
* settle (the 38 decks, rc-meas-ac.sp): -b: 'Warning: can't parse 'vi': ignored', the measure fails ('find(AT) : out of interval', '... failed!'): ngspice cannot evaluate vi() in a measure
V1 in 0 dc 0 ac 1
R1 in out 1k
C1 out 0 1u
.ac lin 1 159.155 159.155
.meas ac vout_ati FIND vi(out) AT=159.155
.control
run
.endc
.end
