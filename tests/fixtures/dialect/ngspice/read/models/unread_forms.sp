unread_forms
* oracle: inpdomod.c (MESA levels 2-4: no IR family), INPfindLev (a level in braces is numparam's before the card is read), inptyplk.c (a device name as type word), inpcompat.c:282 (ako: only in PSPICE mode), create_model (a bare flag), manual 2.15 (a model in an .if branch)
.model ma nmf level=2
.model mb nmos level={lv}
.model mc bsim4 level=54
.model md ako:mm nmos
.model me nmos level=1 pmos
.model mf cpl r=0.1 0 0.1
.model mg nmos vto={temper}
.if (1)
.model mh d
.endif
