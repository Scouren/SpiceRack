tran_brace
* settle: -b: 'Error:  Error: unknown parameter on .tran - ignored', 'in   .tran 0.1u 1u 0 1u/50', 'tran simulation(s) aborted', exit 1: a script command's braces are dropped, not evaluated (F12)
V1 a 0 dc 1 ac 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
C1 b 0 1n
.param p=0.02u
.control
tran 0.1u 1u 0 {1u / 50}
print v(b)[3]
.endc
.end
