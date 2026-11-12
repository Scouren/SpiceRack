unclosed_end
* oracle: settle/unclosed_end.sp: in a block the file ends before closing, .end is a command ngspice has none of (no such command) and the script goes on after it: InvalidInSource at its place
.control
op
.end
echo after
