grid_beh_pwr
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -2.5
va1 a1 0 dc -2
va2 a2 0 dc -1
va3 a3 0 dc -0.5
va4 a4 0 dc 0
va5 a5 0 dc 0.5
va6 a6 0 dc 1
va7 a7 0 dc 2
va8 a8 0 dc 2.5
va9 a9 0 dc 3
.nodeset v(a0)=-2.5 v(a1)=-2 v(a2)=-1 v(a3)=-0.5 v(a4)=0 v(a5)=0.5 v(a6)=1 v(a7)=2 v(a8)=2.5 v(a9)=3
bd266 d266 0 v = pwr(v(a0), v(a1))
bd267 d267 0 v = pwr(v(a0), v(a2))
bd268 d268 0 v = pwr(v(a0), v(a3))
bd269 d269 0 v = pwr(v(a0), v(a4))
bd270 d270 0 v = pwr(v(a0), v(a5))
bd271 d271 0 v = pwr(v(a0), v(a6))
bd272 d272 0 v = pwr(v(a0), v(a7))
bd273 d273 0 v = pwr(v(a0), v(a9))
bd274 d274 0 v = pwr(v(a2), v(a1))
bd275 d275 0 v = pwr(v(a2), v(a2))
bd276 d276 0 v = pwr(v(a2), v(a3))
bd277 d277 0 v = pwr(v(a2), v(a4))
bd278 d278 0 v = pwr(v(a2), v(a5))
bd279 d279 0 v = pwr(v(a2), v(a6))
bd280 d280 0 v = pwr(v(a2), v(a7))
bd281 d281 0 v = pwr(v(a2), v(a9))
bd282 d282 0 v = pwr(v(a3), v(a1))
bd283 d283 0 v = pwr(v(a3), v(a2))
bd284 d284 0 v = pwr(v(a3), v(a3))
bd285 d285 0 v = pwr(v(a3), v(a4))
bd286 d286 0 v = pwr(v(a3), v(a5))
bd287 d287 0 v = pwr(v(a3), v(a6))
bd288 d288 0 v = pwr(v(a3), v(a7))
bd289 d289 0 v = pwr(v(a3), v(a9))
bd290 d290 0 v = pwr(v(a4), v(a4))
bd291 d291 0 v = pwr(v(a4), v(a5))
bd292 d292 0 v = pwr(v(a4), v(a6))
bd293 d293 0 v = pwr(v(a4), v(a7))
bd294 d294 0 v = pwr(v(a4), v(a9))
bd295 d295 0 v = pwr(v(a5), v(a1))
bd296 d296 0 v = pwr(v(a5), v(a2))
bd297 d297 0 v = pwr(v(a5), v(a3))
bd298 d298 0 v = pwr(v(a5), v(a4))
bd299 d299 0 v = pwr(v(a5), v(a5))
bd300 d300 0 v = pwr(v(a5), v(a6))
bd301 d301 0 v = pwr(v(a5), v(a7))
bd302 d302 0 v = pwr(v(a5), v(a9))
bd303 d303 0 v = pwr(v(a6), v(a1))
bd304 d304 0 v = pwr(v(a6), v(a2))
bd305 d305 0 v = pwr(v(a6), v(a3))
bd306 d306 0 v = pwr(v(a6), v(a4))
bd307 d307 0 v = pwr(v(a6), v(a5))
bd308 d308 0 v = pwr(v(a6), v(a6))
bd309 d309 0 v = pwr(v(a6), v(a7))
bd310 d310 0 v = pwr(v(a6), v(a9))
bd311 d311 0 v = pwr(v(a8), v(a1))
bd312 d312 0 v = pwr(v(a8), v(a2))
bd313 d313 0 v = pwr(v(a8), v(a3))
bd314 d314 0 v = pwr(v(a8), v(a4))
bd315 d315 0 v = pwr(v(a8), v(a5))
bd316 d316 0 v = pwr(v(a8), v(a6))
bd317 d317 0 v = pwr(v(a8), v(a7))
bd318 d318 0 v = pwr(v(a8), v(a9))
.op
.end
