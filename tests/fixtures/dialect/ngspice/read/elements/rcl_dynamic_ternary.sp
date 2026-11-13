rcl_dynamic_ternary
* oracle: settle/ternary_braces.sp (R = (time<1u ? {Rl} : {2*Rl}) runs: 100, then 200 after 1u; the braces are parentheses of the B-source value)
.param Rl=100
R4 a 0 R = (time<1u ? {Rl} : {2*Rl})
