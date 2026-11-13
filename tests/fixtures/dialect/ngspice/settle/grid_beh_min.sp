grid_beh_min
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -2.5
va1 a1 0 dc -1
va2 a2 0 dc -0.5
va3 a3 0 dc 0
va4 a4 0 dc 0.5
va5 a5 0 dc 1
va6 a6 0 dc 2.5
.nodeset v(a0)=-2.5 v(a1)=-1 v(a2)=-0.5 v(a3)=0 v(a4)=0.5 v(a5)=1 v(a6)=2.5
bd319 d319 0 v = min(v(a0), v(a0))
bd320 d320 0 v = min(v(a0), v(a1))
bd321 d321 0 v = min(v(a0), v(a2))
bd322 d322 0 v = min(v(a0), v(a3))
bd323 d323 0 v = min(v(a0), v(a4))
bd324 d324 0 v = min(v(a0), v(a5))
bd325 d325 0 v = min(v(a0), v(a6))
bd326 d326 0 v = min(v(a1), v(a0))
bd327 d327 0 v = min(v(a1), v(a1))
bd328 d328 0 v = min(v(a1), v(a2))
bd329 d329 0 v = min(v(a1), v(a3))
bd330 d330 0 v = min(v(a1), v(a4))
bd331 d331 0 v = min(v(a1), v(a5))
bd332 d332 0 v = min(v(a1), v(a6))
bd333 d333 0 v = min(v(a2), v(a0))
bd334 d334 0 v = min(v(a2), v(a1))
bd335 d335 0 v = min(v(a2), v(a2))
bd336 d336 0 v = min(v(a2), v(a3))
bd337 d337 0 v = min(v(a2), v(a4))
bd338 d338 0 v = min(v(a2), v(a5))
bd339 d339 0 v = min(v(a2), v(a6))
bd340 d340 0 v = min(v(a3), v(a0))
bd341 d341 0 v = min(v(a3), v(a1))
bd342 d342 0 v = min(v(a3), v(a2))
bd343 d343 0 v = min(v(a3), v(a3))
bd344 d344 0 v = min(v(a3), v(a4))
bd345 d345 0 v = min(v(a3), v(a5))
bd346 d346 0 v = min(v(a3), v(a6))
bd347 d347 0 v = min(v(a4), v(a0))
bd348 d348 0 v = min(v(a4), v(a1))
bd349 d349 0 v = min(v(a4), v(a2))
bd350 d350 0 v = min(v(a4), v(a3))
bd351 d351 0 v = min(v(a4), v(a4))
bd352 d352 0 v = min(v(a4), v(a5))
bd353 d353 0 v = min(v(a4), v(a6))
bd354 d354 0 v = min(v(a5), v(a0))
bd355 d355 0 v = min(v(a5), v(a1))
bd356 d356 0 v = min(v(a5), v(a2))
bd357 d357 0 v = min(v(a5), v(a3))
bd358 d358 0 v = min(v(a5), v(a4))
bd359 d359 0 v = min(v(a5), v(a5))
bd360 d360 0 v = min(v(a5), v(a6))
bd361 d361 0 v = min(v(a6), v(a0))
bd362 d362 0 v = min(v(a6), v(a1))
bd363 d363 0 v = min(v(a6), v(a2))
bd364 d364 0 v = min(v(a6), v(a3))
bd365 d365 0 v = min(v(a6), v(a4))
bd366 d366 0 v = min(v(a6), v(a5))
bd367 d367 0 v = min(v(a6), v(a6))
.op
.end
