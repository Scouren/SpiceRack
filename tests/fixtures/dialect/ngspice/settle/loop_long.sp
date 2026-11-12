ll: a foreach word that is a long number
* settle (R37, A18; L5 seed 11): -b, numdgt=17: the words 0.12345678901234567890123, 0.123456789012345679 (its 18-digit rounding), 0.12345678901234568 (strtod's shortest) give v(a) = 1.23456789012345677e-01, …705e-01, …691e-01, as the card values of the R1 evidence do (INPevaluate's digits, DIALECT-NGSPICE.md section 3); -73.384558234620672336478e-8 and its two roundings give -7.3384558234620676e-07; alter v1 0.12345678901234567890123 and alter v1 = … give …677e-01: a script number word is read by its digits as written, so a rounded spelling is another number
V1 a 0 1
R1 a 0 1k
.control
set numdgt=17
foreach v 0.12345678901234567890123 0.123456789012345679 0.12345678901234568 -73.384558234620672336478e-8 -73.3845582346206723e-8 -73.384558234620672e-8
alter v1 $v
op
print v(a)
end
alter v1 0.12345678901234567890123
op
print v(a)
alter v1 = 0.12345678901234567890123
op
print v(a)
.endc
.end
