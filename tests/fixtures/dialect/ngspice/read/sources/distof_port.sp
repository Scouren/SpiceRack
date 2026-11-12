distof_port
* oracle: vsrcpar.c VSRC_D_F1/F2 (one value: phase 0), vsrc.c (portnum, z0 under RFSPICE), vsrctemp.c (z0 50 when absent); run `dc 1 portnum 1 z0 100` into 1 ohm = 9.90099e-3
V1 a 0 dc 0 distof1 0.1 distof2 0.2 30
V2 b 0 dc 1 portnum 1 z0 100
R2 b 0 1
V3 c 0 ac 1 portnum 2
R3 c 0 50
.control
op
print v(b) v(c)
.endc
.end
