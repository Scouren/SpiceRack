ils: a .lib call of a section whose body calls another section of its own file
* settle (R71, gf180 sm141064.ngspice): -b: i(v1) = -5.00000e-04 (mim's R1 = 1k*scale = 2k, scale read from the corner section): a section that calls another section of its own file is no cycle; X1 binds
V1 n1 0 1
X1 n1 0 mim
.lib "inc_lib_self.lib" corner
.control
op
print i(v1)
.endc
.end
