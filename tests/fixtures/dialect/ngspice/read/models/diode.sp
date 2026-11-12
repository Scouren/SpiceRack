diode
* oracle: dio.c:49 (level a model parameter; diosetup.c:36 silent is 1, :301 level 3 geometry), dio.c:71 (a diode model's m is its grading coefficient), dio.c instance table (w is an instance default)
.model d1 d is=1e-14 n=1.05
.model d3 d (level=3 is=1e-14 m=0.4 w=1u)
