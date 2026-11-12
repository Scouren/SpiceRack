bsim_version
* oracle: inpdomod.c levels 14 and 54 (version 4.0-4.5 BSIM4v5, 4.6, 4.7, 4.8 or none BSIM4), 73 (1.1, 1.2 HiSIM-HV 1; 2.x or none HiSIM-HV 2), b4set.c:205 (silent version 4.8.3), hsmhv2set.c:137 (2.20); each written version read as the standard version the device runs (b4v5check.c:64: 4.5 is 4.5.0; hsmhvset.c:76-81: HiSIM-HV 1 runs 1.24)
.model n4 nmos level=54 version=4.5 toxe=3n
.model p4 pmos level=14
.model n4b nmos level=54 version=4.8.2
.model hv nmos level=73 version=1.2
.model hv2 pmos level=73
