iso: .lib file section, a subcircuit of another section of the file
* settle (R71 case 3): -b: 'Error: unknown subckt: x1 n1 other', exit 1: another section of the same file is not read
.lib inc_sections.lib s1
V1 n1 0 1
X1 n1 other
.control
op
print i(v1)
.endc
.end
