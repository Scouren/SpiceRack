dc_brace
* settle: -b: rows at 0 and 1: {1} is read as 1, the braces dropped
V1 a 0 dc 1 ac 1 pulse(0 1 0 1n 1n 0.5u 1u)
R1 a b 1k
C1 b 0 1n
.param p=0.02u
.control
dc v1 0 {1} 1
print v(b)
.endc
.end
