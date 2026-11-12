s1: what reset keeps and discards
* settle (S3-CONTROL S1): -b: v(b) = 0.4988 (deck), 0.3185 (alter r1 2k, altermod is=1e-10; alterparam p=3k not yet applied),
* after reset 0.6114: r1 back to 1k, is back to 1e-14, p = 3k kept; reltol variable gone; show r1 resistance 1000, r2 3000
.param p=1k
V1 a 0 1
R1 a b 1k
R2 b 0 {p} tc1=0.01
D1 b 0 dmod
.model dmod d is=1e-14
.control
op
print v(b)
alter r1 2k
altermod dmod is=1e-10
alterparam p=3k
option reltol=0.01
op
print v(b)
echo "reltol: $reltol"
reset
op
print v(b)
echo "reltol after reset: $reltol"
show r1 : resistance
showmod dmod : is
show r2 : resistance
.endc
.end
