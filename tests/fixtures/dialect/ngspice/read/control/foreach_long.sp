foreach_long
* oracle: settle/loop_long.sp (a script number word is its digits as written: 0.12345678901234567890123 and its 18-digit rounding run as different values); R37, A18; L5 seed 11
v1 a 0 1
.control
foreach v 0.12345678901234567890123 -73.384558234620672336478e-8 1k
alter v1 $v
op
end
alter v1 0.12345678901234567890123
.endc
.end
