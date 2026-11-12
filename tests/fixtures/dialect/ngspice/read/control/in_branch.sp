in_branch
* oracle: lead's Q8: a block in an .if branch runs whatever the condition (settle/order_block_placement.sp): Private
.param s=1
.if (s == 1)
.control
op
.endc
.endif
.end
