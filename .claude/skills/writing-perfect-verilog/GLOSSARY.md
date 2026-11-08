# Glossary

One-line anchors for the **bold terms** in [`SKILL.md`](SKILL.md); the working detail lives in each term's section there.

**Readability** — a reader reconstructs the whole module in one top-down pass; in RTL the legible structure and the correct one are the same structure.

**Downflow** — declared above driven, driven above consumed; a name's origin is always behind the reader.

**`_d`/`_q` convention** — `_q` leaves the flop (registered value), `_d` enters it on the next edge; the suffix carries timing.

**Latch** — memory the reader didn't ask for. When a combinational path leaves a signal unassigned on some branch, synthesis must preserve the old value, so instead of pure logic it infers a *level-sensitive* latch — transparent while the enabling condition holds, holding otherwise — with the timing, testability, and glitch problems that entails. Prevented entirely by the defaults rule.

**defaults** — every `always_comb` opens by assigning every signal it drives, so no path can leave one unassigned.

**Blocking (`=`) vs nonblocking (`<=`)** — `=` updates immediately in order (combinational); `<=` updates all left-hand sides at the end of the time step, modelling real registers (sequential).

**Two-block FSM** — one `always_ff` for the state register, one `always_comb` for next-state, outputs derived separately.

**Registered outputs** — outputs taken from a flop: one cycle of latency buys glitch-free, timing-clean edges.

**CDC (clock-domain crossing)** — a signal sampled in a domain other than the one that drove it; unsynchronized, it goes metastable in silicon while simulating clean.

**No-op** — a comment the structure already states; if deleting it loses no intent, delete it.

**Scatter** — one signal's logic fragmented across blocks; the root failure the others are shapes of.

**Suffix rot** — a reserved suffix used loosely until it signals nothing; the convention pays only if absolute.
