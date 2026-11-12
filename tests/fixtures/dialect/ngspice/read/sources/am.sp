am
* oracle: vsrcload.c AM (VO VMO VMA FM FC TD PHM PHC; 0 for t - TD <= 0, op too); run am(0.5 1 0.5 1k 10k 0 0 90) op 0
V1 a 0 am(0.5 1 0.5 1k 10k 0 0 90)
V2 b 0 am(0 1)
.control
op
print v(a) v(b)
tran 1u 100u
print v(a)[30] v(b)[30]
.endc
.end
