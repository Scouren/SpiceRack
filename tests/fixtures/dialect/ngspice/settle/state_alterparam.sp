p1: alterparam on a multi-parameter .param line; foreach + alterparam + reset leaves the last value; continuation lines; *# lines
* settle (S3-CONTROL S4): -b: foreach over 1k 3k with alterparam+reset: 0.3333, 0.6; after the loop 0.6 and after a reset 0.6 (the last value stays, KeepLast)
* 'alterparam p = 4k' then 'p=5k' + reset: 0.7143 (the last wins); q=2k of the same .param line kept; '+' continues a script line; a '*#' line outside .control is a command (printed first)
.param p=1k q=2k
V1 a 0 1
R1 a b {q}
R2 b 0 {p}
*# echo "hash command outside control"
.control
foreach val 1k
+ 3k
echo "val=$val"
alterparam p=$val
reset
op
print v(b)
end
echo "after loop"
op
print v(b)
reset
op
print v(b)
alterparam p = 4k
alterparam p=5k
reset
op
print v(b)
.endc
.end
