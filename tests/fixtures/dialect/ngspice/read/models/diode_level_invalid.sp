diode_level_invalid
* oracle: diotemp.c:210 (a level other than 1 and 3 scales the breakdown current by the area as level 3 does, without its geometry: neither family: Private, the S3.2 ruling 7), run: d level=2 area=3 gives the level-3 current 7.45687e+05, level 1 2.48562e+05
.model d2 d level=2 is=1e-14
.model d200 d level=200
