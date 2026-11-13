script_mixed
r1 a 0 1k
.model dmod d
.options reltol=1e-4
.control
alter r1 2k
op
echo x
altermod dmod is=1e-10
tran 1u 5u
.endc
.end
