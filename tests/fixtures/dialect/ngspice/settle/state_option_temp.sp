s2: option, set temp across reset
* settle (S3-CONTROL S2): -b: v(b) 0.5305 (.temp 40), 0.5516 (option temp=50), 0.5305 after reset, 0.5708 (set temp=60), 0.5305 after reset;
* 'option' after: temp 313.15 K, abstol 1e-13 (.options kept), reltol 0.001 (script option discarded). option = set (com_option.c cp_vset; options.c cp_usrset US_SIMVAR)
V1 a 0 1
R1 a b 1k
R2 b 0 1k tc1=0.01
.options abstol=1e-13
.temp 40
.control
op
print v(b)
option reltol=0.01 temp=50
op
print v(b)
reset
op
print v(b)
set temp=60
op
print v(b)
reset
op
print v(b)
option
.endc
.end
