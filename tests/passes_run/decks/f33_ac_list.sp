f33_ac_list: ngspice-45 rejects `.ac list 1k 2k` (Error on line 5, circuit not parsed): .ac has no list form
v1 in 0 dc 1 ac 1
r1 in out 1k
c1 out 0 1u
.ac list 1k 2k
.end
