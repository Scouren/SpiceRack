vdmos
* oracle: inpcom.c inp_vdmos_model (a type word starting vdmos; pchan anywhere or vdmosp is p; pchan, nchan, ron=, vds=, qg=, mfg= dropped), vdmos.c model table
.model v1 vdmos vto=2 kp=10
.model v2 vdmos pchan vto=-2 kp=10 ron=5 mfg=acme
.model v3 vdmosp (vto=-2)
.model v4 vdmosn vto=2 qg=1n
