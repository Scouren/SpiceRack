isn: .lib file section, a subcircuit of the named section
* settle (R71 case 3): -b: i(v1) = -5.00000e-04: a .lib file section call reads the named section
.lib inc_sections.lib s1
V1 n1 0 1
X1 n1 sub
.control
op
print i(v1)
.endc
.end
