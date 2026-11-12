script_param_loop
.param p=1k
.control
foreach val 1k 3k
alterparam p=$val
reset
op
end
.endc
.end
