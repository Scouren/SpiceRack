da: a D card with a number after its model
* settle: -b: 'could not find a valid modelname', 'Simulation interrupted due to error!', exit 1; the same for 2, 2.0 and 4e-1 in its place (default ngspice: a D card takes no unlabeled area; the paranoia/pton decks that do run under ngbehavior=ps)
V1 a 0 0.7
D1 a 0 dm 400E-12
.model dm d is=1e-14
.control
op
print i(v1)
.endc
.end
