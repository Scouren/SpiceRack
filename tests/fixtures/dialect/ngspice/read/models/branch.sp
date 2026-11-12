branch
* oracle: ngspice manual 2.15 (models in .if branches), RULINGS R33 (a definition in a branch is visible in that branch only); Placed(Branch, Model) pending in AMEND-A12 A15
.if (1)
.model mh d is=1e-14
.else
.model mh d is=1e-12
.endif
