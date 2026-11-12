numbers_long
* oracle: R37, A18 (a number longer than 18 significant digits is kept as written: ngspice reads the digits, INPevaluate and fetchnumber, DIALECT-NGSPICE.md section 3 numbers)
.param a=1234567890123456789012 b={-0.0012345678901234567890e3k} c=1.234567890123456789u
r1 n 0 1.00000000000000000001k
