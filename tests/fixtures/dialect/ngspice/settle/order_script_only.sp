o6: script op only, no dot analysis, -r
* settle (S3-CONTROL O6): -b -r with only a script 'op': v(b) printed, no raw file written (dosim unlinks an empty raw)
V1 a 0 1
R1 a b 1k
R2 b 0 1k
.control
op
print v(b)
.endc
.end
