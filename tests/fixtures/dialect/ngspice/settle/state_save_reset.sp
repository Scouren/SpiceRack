p3: does a script save survive reset; .save card after reset
* settle (S3-CONTROL S5): -b: a script 'save v(b)' holds for op1 (vector b only); after reset op2 has a, b, v1#branch: reset discards script saves (runcoms2.c com_remcirc dbfree)
V1 a 0 1
R1 a b 1k
R2 b 0 1k
.control
save v(b)
op
echo "op1 vectors:"
display
reset
op
echo "op2 vectors:"
display
.endc
.end
