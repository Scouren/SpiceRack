subckt_nested
* oracle: manual 2.4.3 (.subckt nests, params:), INV-constructs B1-B4, INV-semantics 3.06 (nearest declaration), 11.01 (gnd is 0)
.param w=9
.subckt outer a GND params: w=1
.param k={w+1}
.subckt inner x
.param j={w*k}
.ends
.ends outer
