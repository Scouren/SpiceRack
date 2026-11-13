grid_beh_pow
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
bd213 d213 0 v = pow(v(a0), v(a1))
bd214 d214 0 v = pow(v(a0), v(a2))
bd215 d215 0 v = pow(v(a0), v(a3))
bd216 d216 0 v = pow(v(a0), v(a4))
bd217 d217 0 v = pow(v(a0), v(a5))
bd218 d218 0 v = pow(v(a0), v(a6))
bd219 d219 0 v = pow(v(a0), v(a7))
bd220 d220 0 v = pow(v(a0), v(a9))
bd221 d221 0 v = pow(v(a2), v(a1))
bd222 d222 0 v = pow(v(a2), v(a2))
bd223 d223 0 v = pow(v(a2), v(a3))
bd224 d224 0 v = pow(v(a2), v(a4))
bd225 d225 0 v = pow(v(a2), v(a5))
bd226 d226 0 v = pow(v(a2), v(a6))
bd227 d227 0 v = pow(v(a2), v(a7))
bd228 d228 0 v = pow(v(a2), v(a9))
bd229 d229 0 v = pow(v(a3), v(a1))
bd230 d230 0 v = pow(v(a3), v(a2))
bd231 d231 0 v = pow(v(a3), v(a3))
bd232 d232 0 v = pow(v(a3), v(a4))
bd233 d233 0 v = pow(v(a3), v(a5))
bd234 d234 0 v = pow(v(a3), v(a6))
bd235 d235 0 v = pow(v(a3), v(a7))
bd236 d236 0 v = pow(v(a3), v(a9))
bd237 d237 0 v = pow(v(a4), v(a4))
bd238 d238 0 v = pow(v(a4), v(a5))
bd239 d239 0 v = pow(v(a4), v(a6))
bd240 d240 0 v = pow(v(a4), v(a7))
bd241 d241 0 v = pow(v(a4), v(a9))
bd242 d242 0 v = pow(v(a5), v(a1))
bd243 d243 0 v = pow(v(a5), v(a2))
bd244 d244 0 v = pow(v(a5), v(a3))
bd245 d245 0 v = pow(v(a5), v(a4))
bd246 d246 0 v = pow(v(a5), v(a5))
bd247 d247 0 v = pow(v(a5), v(a6))
bd248 d248 0 v = pow(v(a5), v(a7))
bd249 d249 0 v = pow(v(a5), v(a9))
bd250 d250 0 v = pow(v(a6), v(a1))
bd251 d251 0 v = pow(v(a6), v(a2))
bd252 d252 0 v = pow(v(a6), v(a3))
bd253 d253 0 v = pow(v(a6), v(a4))
bd254 d254 0 v = pow(v(a6), v(a5))
bd255 d255 0 v = pow(v(a6), v(a6))
bd256 d256 0 v = pow(v(a6), v(a7))
bd257 d257 0 v = pow(v(a6), v(a9))
bd258 d258 0 v = pow(v(a8), v(a1))
bd259 d259 0 v = pow(v(a8), v(a2))
bd260 d260 0 v = pow(v(a8), v(a3))
bd261 d261 0 v = pow(v(a8), v(a4))
bd262 d262 0 v = pow(v(a8), v(a5))
bd263 d263 0 v = pow(v(a8), v(a6))
bd264 d264 0 v = pow(v(a8), v(a7))
bd265 d265 0 v = pow(v(a8), v(a9))
.op
.end
