grid_beh_caret
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
bd580 d580 0 v = (v(a0))^(v(a1))
bd581 d581 0 v = (v(a0))^(v(a2))
bd582 d582 0 v = (v(a0))^(v(a3))
bd583 d583 0 v = (v(a0))^(v(a4))
bd584 d584 0 v = (v(a0))^(v(a5))
bd585 d585 0 v = (v(a0))^(v(a6))
bd586 d586 0 v = (v(a0))^(v(a7))
bd587 d587 0 v = (v(a0))^(v(a9))
bd588 d588 0 v = (v(a2))^(v(a1))
bd589 d589 0 v = (v(a2))^(v(a2))
bd590 d590 0 v = (v(a2))^(v(a3))
bd591 d591 0 v = (v(a2))^(v(a4))
bd592 d592 0 v = (v(a2))^(v(a5))
bd593 d593 0 v = (v(a2))^(v(a6))
bd594 d594 0 v = (v(a2))^(v(a7))
bd595 d595 0 v = (v(a2))^(v(a9))
bd596 d596 0 v = (v(a3))^(v(a1))
bd597 d597 0 v = (v(a3))^(v(a2))
bd598 d598 0 v = (v(a3))^(v(a3))
bd599 d599 0 v = (v(a3))^(v(a4))
bd600 d600 0 v = (v(a3))^(v(a5))
bd601 d601 0 v = (v(a3))^(v(a6))
bd602 d602 0 v = (v(a3))^(v(a7))
bd603 d603 0 v = (v(a3))^(v(a9))
bd604 d604 0 v = (v(a4))^(v(a4))
bd605 d605 0 v = (v(a4))^(v(a5))
bd606 d606 0 v = (v(a4))^(v(a6))
bd607 d607 0 v = (v(a4))^(v(a7))
bd608 d608 0 v = (v(a4))^(v(a9))
bd609 d609 0 v = (v(a5))^(v(a1))
bd610 d610 0 v = (v(a5))^(v(a2))
bd611 d611 0 v = (v(a5))^(v(a3))
bd612 d612 0 v = (v(a5))^(v(a4))
bd613 d613 0 v = (v(a5))^(v(a5))
bd614 d614 0 v = (v(a5))^(v(a6))
bd615 d615 0 v = (v(a5))^(v(a7))
bd616 d616 0 v = (v(a5))^(v(a9))
bd617 d617 0 v = (v(a6))^(v(a1))
bd618 d618 0 v = (v(a6))^(v(a2))
bd619 d619 0 v = (v(a6))^(v(a3))
bd620 d620 0 v = (v(a6))^(v(a4))
bd621 d621 0 v = (v(a6))^(v(a5))
bd622 d622 0 v = (v(a6))^(v(a6))
bd623 d623 0 v = (v(a6))^(v(a7))
bd624 d624 0 v = (v(a6))^(v(a9))
bd625 d625 0 v = (v(a8))^(v(a1))
bd626 d626 0 v = (v(a8))^(v(a2))
bd627 d627 0 v = (v(a8))^(v(a3))
bd628 d628 0 v = (v(a8))^(v(a4))
bd629 d629 0 v = (v(a8))^(v(a5))
bd630 d630 0 v = (v(a8))^(v(a6))
bd631 d631 0 v = (v(a8))^(v(a7))
bd632 d632 0 v = (v(a8))^(v(a9))
.op
.end
