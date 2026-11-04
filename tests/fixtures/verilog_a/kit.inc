// A made-up Spectre kit: a Verilog-A varactor, a subckt wrapping it, and
// one instance of each at the top. No real PDK data. `.inc`, not `.scs`:
// the golden gate's fixture walk (src/bin/golden/inputs.rs) reads every
// `tests/**/*.scs`, and this kit is no gate input.
simulator lang=spectre
ahdl_include "va/varactor.va"
subckt varcap (p n)
parameters area=1p
xv (p n) varactor_va area=area c0=2f
ends varcap
xtop (a 0) varactor_va area=4p
xcap (a 0) varcap area=2p
