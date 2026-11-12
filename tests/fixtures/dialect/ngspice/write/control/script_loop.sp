script_loop
.control
foreach v 1k 2.2k
alter r1 $v
op
echo $v
end
.endc
.end
