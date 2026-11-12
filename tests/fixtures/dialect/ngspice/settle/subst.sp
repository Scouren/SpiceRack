v1: what text ngspice substitutes for $v, $&v, $#v; loop variable kind
* settle (S3-CONTROL V1): -b: set x=1.23456789k gives $x = 1234.57; $&k = 1234.57, 0.333333, 1.23457E+11, 1.5E-17 (variable.c %G);
* $#lst = 3, $lst[1] = 1000 (a set list's number reprinted); foreach v gives the word as written: 1k, 2.2k, 0.5 (control.c:465 CP_STRING);
* after the foreach r1 = 0.5 (the last value); alter r1 $&k gives v(b) = 0.447513 (1234.57 ohm)
V1 a 0 1
R1 a b 1k
R2 b 0 1k
.control
set x = 1.23456789k
echo "x=[$x]"
let k = 1.23456789k
echo "k=[$&k]"
let k2 = 1/3
echo "k2=[$&k2]"
let big = 123456789012
echo "big=[$&big]"
let tiny = 1.5e-17
echo "tiny=[$&tiny]"
set lst = ( 1k 2.5meg 3 )
echo "count=[$#lst] first=[$lst[1]]"
foreach v 1k 2.2k 0.5
echo "v=[$v]"
alter r1 $v
op
print v(b)
end
show r1 : resistance
foreach w 1k 2k
let ww = $w
echo "ww=[$&ww]"
end
alter r1 $&k
op
print v(b)
.endc
.end
