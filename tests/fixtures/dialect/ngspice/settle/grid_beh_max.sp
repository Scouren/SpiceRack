grid_beh_max
* settle grid: values in the raw file (ngspice -b -r), compared with the IR's reference evaluator by tests/dialect_ngspice_grid.rs
va0 a0 0 dc -2.5
va1 a1 0 dc -1
va2 a2 0 dc -0.5
va3 a3 0 dc 0
va4 a4 0 dc 0.5
va5 a5 0 dc 1
va6 a6 0 dc 2.5
.nodeset v(a0)=-2.5 v(a1)=-1 v(a2)=-0.5 v(a3)=0 v(a4)=0.5 v(a5)=1 v(a6)=2.5
bd368 d368 0 v = max(v(a0), v(a0))
bd369 d369 0 v = max(v(a0), v(a1))
bd370 d370 0 v = max(v(a0), v(a2))
bd371 d371 0 v = max(v(a0), v(a3))
bd372 d372 0 v = max(v(a0), v(a4))
bd373 d373 0 v = max(v(a0), v(a5))
bd374 d374 0 v = max(v(a0), v(a6))
bd375 d375 0 v = max(v(a1), v(a0))
bd376 d376 0 v = max(v(a1), v(a1))
bd377 d377 0 v = max(v(a1), v(a2))
bd378 d378 0 v = max(v(a1), v(a3))
bd379 d379 0 v = max(v(a1), v(a4))
bd380 d380 0 v = max(v(a1), v(a5))
bd381 d381 0 v = max(v(a1), v(a6))
bd382 d382 0 v = max(v(a2), v(a0))
bd383 d383 0 v = max(v(a2), v(a1))
bd384 d384 0 v = max(v(a2), v(a2))
bd385 d385 0 v = max(v(a2), v(a3))
bd386 d386 0 v = max(v(a2), v(a4))
bd387 d387 0 v = max(v(a2), v(a5))
bd388 d388 0 v = max(v(a2), v(a6))
bd389 d389 0 v = max(v(a3), v(a0))
bd390 d390 0 v = max(v(a3), v(a1))
bd391 d391 0 v = max(v(a3), v(a2))
bd392 d392 0 v = max(v(a3), v(a3))
bd393 d393 0 v = max(v(a3), v(a4))
bd394 d394 0 v = max(v(a3), v(a5))
bd395 d395 0 v = max(v(a3), v(a6))
bd396 d396 0 v = max(v(a4), v(a0))
bd397 d397 0 v = max(v(a4), v(a1))
bd398 d398 0 v = max(v(a4), v(a2))
bd399 d399 0 v = max(v(a4), v(a3))
bd400 d400 0 v = max(v(a4), v(a4))
bd401 d401 0 v = max(v(a4), v(a5))
bd402 d402 0 v = max(v(a4), v(a6))
bd403 d403 0 v = max(v(a5), v(a0))
bd404 d404 0 v = max(v(a5), v(a1))
bd405 d405 0 v = max(v(a5), v(a2))
bd406 d406 0 v = max(v(a5), v(a3))
bd407 d407 0 v = max(v(a5), v(a4))
bd408 d408 0 v = max(v(a5), v(a5))
bd409 d409 0 v = max(v(a5), v(a6))
bd410 d410 0 v = max(v(a6), v(a0))
bd411 d411 0 v = max(v(a6), v(a1))
bd412 d412 0 v = max(v(a6), v(a2))
bd413 d413 0 v = max(v(a6), v(a3))
bd414 d414 0 v = max(v(a6), v(a4))
bd415 d415 0 v = max(v(a6), v(a5))
bd416 d416 0 v = max(v(a6), v(a6))
.op
.end
