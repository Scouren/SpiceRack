script_pairs
.control
set x=1
echo a
echo b
.endc
.param p=1
.control
run
.endc
.control
op
.endc
.control
if 1
op
end
.endc
.end
