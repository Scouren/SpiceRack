ob: option with a brace value
* settle (S3-CONTROL S3): -b: 'Error: bad type given for option temp --' and v(b) = 0.5 (27 C: option temp={q} is not read); option temp={10+40} gives 0.4535519 (10 C: the frontend reads the digits before '+', not the expression); option temp=50 gives 0.5515695 (50 C). A script option's value is the frontend's number, never a numparam expression
.param q=50
V1 a 0 1
R1 a b 1k
R2 b 0 1k tc1=0.01
.control
option temp={q}
op
print v(b)
option temp={10+40}
op
print v(b)
option temp=50
op
print v(b)
option reltol=1m
option
.endc
.end
