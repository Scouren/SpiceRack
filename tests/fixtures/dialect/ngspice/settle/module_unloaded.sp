f11: a module model with nothing that loads its module
* settle: -b: 'Unknown model type sp_mos2 - ignored', 'Unable to find definition of model nm', 'Simulation interrupted due to error!', exit 1 (F11)
V1 in 0 1
V2 out 0 1
.model nm sp_mos2 type=1 phi=0.7
nm1 out in 0 0 nm w=200u l=50u nrs=1 nrd=1 m=2
.control
op
print i(v2)
.endc
.end
