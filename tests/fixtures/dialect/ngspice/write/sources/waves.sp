waves
v1 a 0 dc 1 ac 2 pulse(0 1 0 1n 0 5u 10u 3)
i2 b 0 sffm(0 1 1k 2 10k 30 45) m=2
v3 c 0 sffm(0 1 1k 2 10k 1u)
v4 d 0 pwl(0 0 1u {k * 2}) td=2u r=0
v5 e 0 trrandom(4 1u 0 2)
v6 f 0 am(0 1 0.5 1k)
v7 g 0 dc 0 ac 1 90 distof1 0.1 portnum 1 z0=50
v8 h 0 sin(0 1 1k 0 1k)
v9 i 0 exp(0 1 0 1u)
.end
