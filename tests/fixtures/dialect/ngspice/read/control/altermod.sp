altermod
* oracle: S3-CONTROL S4: device.c:1194 com_altermod (mod p = v, mod p v, @mod[p] = v); settle/state_reset.sp
.model dmod d
.control
altermod dmod is=1e-10
altermod dmod bf 50
altermod @dmod[n]=1.5
op
.endc
.end
