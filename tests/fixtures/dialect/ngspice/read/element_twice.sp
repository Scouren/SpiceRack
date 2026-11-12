element_twice
* oracle: ngspice-45 run (scratchpad runs/dup: two cards r1, b2 or x1 in one scope: "device already exists, bail out"); names fold case (INV 3.02)
r1 a 0 1k
R1 a 0 2k
.subckt s a
r1 a 0 1
.ends
