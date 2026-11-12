s3: set temp and option temp across reset with no .temp card
* settle (S3-CONTROL S3): -b: set temp=60 gives 0.5708; after reset 0.5 (27 C): a script temperature is discarded by reset with no .temp card too
V1 a 0 1
R1 a b 1k
R2 b 0 1k tc1=0.01
.control
set temp=60
op
print v(b)
reset
op
print v(b)
option temp=50
reset
op
print v(b)
unset temp
reset
op
print v(b)
.endc
.end
