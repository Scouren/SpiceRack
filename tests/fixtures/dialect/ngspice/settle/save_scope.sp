sc1: what a script save scopes over
* settle: -b, display: op1 holds b, before and after the later 'save all'; ac1 holds a, b, frequency, v1#branch; after 'delete all' and 'save v(b)' op2 holds b: in memory a save holds from where it stands, the list grows until 'delete all'. The raw files written by 'write' (s1.raw after op1, s3.raw after op2) hold v(all) beside v(b) (loaded back with load and display), the save list read again at write time
V1 a 0 dc 1 ac 1
R1 a b 1k
C1 b 0 1n
.control
save v(b)
op
echo "== op1 after op"
display
write s1.raw
save all
echo "== op1 after save all"
display
ac lin 1 1k 1k
echo "== ac1"
display
delete all
save v(b)
op
echo "== op2"
display
write s3.raw
.endc
.end
