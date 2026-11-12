script_loop
r1 a 0 1k
.control
foreach v 1k 2.2k
alter r1 $v
op
echo $v
end
.endc
.end
