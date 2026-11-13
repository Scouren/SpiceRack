noise_brace
* settle: -b: 'Warning: Noise measurement at a single frequency 1000 only!', exit 0: {2*1k} is read as 2 (a single frequency), a wrong analysis with a warning
V1 a 0 dc 1 ac 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
C1 b 0 1n
.param p=0.02u
.control
noise v(b) v1 lin 1 1k {2*1k}
print all
.endc
.end
