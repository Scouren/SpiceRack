grid_beh_pow_op
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
bd527 d527 0 v = (v(a0))**(v(a1))
bd528 d528 0 v = (v(a0))**(v(a2))
bd529 d529 0 v = (v(a0))**(v(a3))
bd530 d530 0 v = (v(a0))**(v(a4))
bd531 d531 0 v = (v(a0))**(v(a5))
bd532 d532 0 v = (v(a0))**(v(a6))
bd533 d533 0 v = (v(a0))**(v(a7))
bd534 d534 0 v = (v(a0))**(v(a9))
bd535 d535 0 v = (v(a2))**(v(a1))
bd536 d536 0 v = (v(a2))**(v(a2))
bd537 d537 0 v = (v(a2))**(v(a3))
bd538 d538 0 v = (v(a2))**(v(a4))
bd539 d539 0 v = (v(a2))**(v(a5))
bd540 d540 0 v = (v(a2))**(v(a6))
bd541 d541 0 v = (v(a2))**(v(a7))
bd542 d542 0 v = (v(a2))**(v(a9))
bd543 d543 0 v = (v(a3))**(v(a1))
bd544 d544 0 v = (v(a3))**(v(a2))
bd545 d545 0 v = (v(a3))**(v(a3))
bd546 d546 0 v = (v(a3))**(v(a4))
bd547 d547 0 v = (v(a3))**(v(a5))
bd548 d548 0 v = (v(a3))**(v(a6))
bd549 d549 0 v = (v(a3))**(v(a7))
bd550 d550 0 v = (v(a3))**(v(a9))
bd551 d551 0 v = (v(a4))**(v(a4))
bd552 d552 0 v = (v(a4))**(v(a5))
bd553 d553 0 v = (v(a4))**(v(a6))
bd554 d554 0 v = (v(a4))**(v(a7))
bd555 d555 0 v = (v(a4))**(v(a9))
bd556 d556 0 v = (v(a5))**(v(a1))
bd557 d557 0 v = (v(a5))**(v(a2))
bd558 d558 0 v = (v(a5))**(v(a3))
bd559 d559 0 v = (v(a5))**(v(a4))
bd560 d560 0 v = (v(a5))**(v(a5))
bd561 d561 0 v = (v(a5))**(v(a6))
bd562 d562 0 v = (v(a5))**(v(a7))
bd563 d563 0 v = (v(a5))**(v(a9))
bd564 d564 0 v = (v(a6))**(v(a1))
bd565 d565 0 v = (v(a6))**(v(a2))
bd566 d566 0 v = (v(a6))**(v(a3))
bd567 d567 0 v = (v(a6))**(v(a4))
bd568 d568 0 v = (v(a6))**(v(a5))
bd569 d569 0 v = (v(a6))**(v(a6))
bd570 d570 0 v = (v(a6))**(v(a7))
bd571 d571 0 v = (v(a6))**(v(a9))
bd572 d572 0 v = (v(a8))**(v(a1))
bd573 d573 0 v = (v(a8))**(v(a2))
bd574 d574 0 v = (v(a8))**(v(a3))
bd575 d575 0 v = (v(a8))**(v(a4))
bd576 d576 0 v = (v(a8))**(v(a5))
bd577 d577 0 v = (v(a8))**(v(a6))
bd578 d578 0 v = (v(a8))**(v(a7))
bd579 d579 0 v = (v(a8))**(v(a9))
.op
.end
