tran_num
* settle: -b: v(b)[3] = 8.999840e-07: the neighbour, a number
V1 a 0 dc 1 ac 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
C1 b 0 1n
.param p=0.02u
.control
tran 0.1u 1u 0 0.02u
print v(b)[3]
.endc
.end
