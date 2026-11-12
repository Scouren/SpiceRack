foreach_inert_var
* oracle: lead's Q3: an inert command reading the loop variable stays in the body as written
.control
foreach v 1k 2k
alter r1 $v
op
echo $v
end
.endc
.end
