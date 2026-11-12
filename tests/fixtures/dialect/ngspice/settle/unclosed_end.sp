e1: .end inside a block the file ends before closing
* settle (S3-CONTROL O1): -b: 'Warning: Missing .endc statement!', then '.end: no such command available in ngspice', v(a) = 1, 'after-end': in a block the file ends before closing, .end is a script command ngspice has none of, and the script goes on
V1 a 0 1
R1 a 0 1k
.control
op
print v(a)
.end
echo after-end
