sc2: save list echoed between commands
* settle: -b: status lists '1 save b', then '1 save b' '2 save all', nothing after 'delete all', then '3 save a'; op2 holds a only: 'delete all' empties the save list
V1 a 0 dc 1 ac 1
R1 a b 1k
C1 b 0 1n
.control
save v(b)
status
op
save all
status
delete all
status
save v(a)
status
op
echo "== op2"
display
.endc
.end
