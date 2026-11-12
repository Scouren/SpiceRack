o5: .op card, script alters with no run: does the post-script run see the alter?
* settle (S3-CONTROL O5): -b -r: the .op card runs after the script on its final state: raw v(b) = 0.25 (r1 = 3k)
V1 a 0 1
R1 a b 1k
R2 b 0 1k
.op
.control
alter r1 3k
.endc
.end
