bjt
* oracle: inpdomod.c npn/pnp switch (levels 0-2 BJT, 4 and 9 VBIC, 8 hicum2), vbicsetup.c:360 (a silent vers is 1.2), hicum2setup.c:45 (silent version 2.4.0), run: level=2 gives the level-1 current
.model q1 npn bf=100 is=1e-16
.model q2 pnp level=2 bf=50
.model q3 npn level=4 rbi=10
.model q4 pnp level=9 is=1e-16 vers=1.2
.model q5 npn level=8 c10=2e-30
