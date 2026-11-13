dup_section
* settle -b: 'v(n) = 2.000000e+00' (the first section s of dup_section.lib, p=2), no warning: ngspice reads the first .lib section of a name, R1001
.lib dup_section.lib s
v1 n 0 {p}
r1 n 0 1
.control
op
print v(n)
.endc
.end
