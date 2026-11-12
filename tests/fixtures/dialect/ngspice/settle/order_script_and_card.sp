o7: .op card, script op then alter then run; -r raw and memory
* settle (S3-CONTROL O7): -b -r: script op v(b)=0.5, alter, run v(b)=0.25; raw: one plot, v(b) = 0.25 (the post-script run, on the altered circuit)
V1 a 0 1
R1 a b 1k
R2 b 0 1k
.op
.control
op
print v(b)
alter r1 3k
run
print v(b)
.endc
.end
