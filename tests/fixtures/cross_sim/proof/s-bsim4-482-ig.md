# S-BSIM4-482-IG — third-simulator (Xyce 7.10 is the outlier)

`BSIM4_v4p82/test1`, `test4`, `test5` (xyce-ngspice): the gate current of
BSIM4 4.8.2. VACASK's `bsim4v8.osdi` against ngspice-45: Ig max relative
difference 3.8e-7. Xyce 7.10 against both: 1.22 %. Id agrees to 5e-6 on all
three simulators. Two of three agree; Xyce's Ig is the outlier. The other
`BSIM4_v4p82` decks (test2, 3, 6, 7, 9, 10, 13, 14) are not settled by
this run and stay mismatches.

Source: research/settle-ngspice.md, mismatches row S-BSIM4-482-IG;
ZERO-LEDGER P1-045.
