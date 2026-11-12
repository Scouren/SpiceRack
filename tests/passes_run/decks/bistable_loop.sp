bistable_loop: the same circuit and values by foreach, alter, op: each op starts afresh (ngspice-45 at -2 .. 1 V: -2, 1.611, 1.059, 0.527, 0, -0.527, -1.059: the middle, unstable root from -1.5 V on)
vin in 0 dc 0
b1 0 out i=2m*tanh(10*(v(out)+v(in)))
r1 out 0 1k
.control
foreach val -2 -1.5 -1 -0.5 0 0.5 1
  alter vin dc = $val
  op
  print v(out)
end
.endc
.end
