ac_brace
* settle: -b: 'Warning, ngspice assumes default parameter(s) for ac simulation', 'Check your input line '.ac lin 1 1k 2*1k'', vm(b) = 9.999803e-01, exit 0: the braces dropped, the field misread with a warning
V1 a 0 dc 1 ac 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
C1 b 0 1n
.param p=0.02u
.control
ac lin 1 1k {2*1k}
print vm(b)
.endc
.end
