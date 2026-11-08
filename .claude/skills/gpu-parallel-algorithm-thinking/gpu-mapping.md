# GPU Mapping

Reached from step 8, after a candidate is chosen. Answer per stage.

## Ownership

Pick the level that owns one unit of work: thread · warp/wavefront · block/workgroup ·
persistent block · grid-wide phase · one GPU · multiple GPUs.

```
What does one thread own?
What does one warp own?
What does one block own?
What data is reused, and at which level?
Where is synchronization required?
Where is communication required?
What is the unit of load balancing?
```

A stage whose answers are all "one thread, nothing reused" is a map — cheap, and a hint the
surrounding stages carry the difficulty.

## Synchronization ladder

Choose the smallest scope that suffices:

1. independent threads
2. warp-level exchange (shuffle, ballot, vote)
3. block barrier + shared memory
4. separate kernel launches
5. grid-wide synchronization (cooperative groups)
6. host synchronization

Moving up a rung is a cost decision that belongs in the comparison table.

## Memory placement

State what is reused and what moves, per stage, per element — naming the space (register ·
shared · L1/L2 · global · constant · host-pinned · unified) is not the answer. Placement
mechanics — coalescing, vectorized loads, bank-conflict padding — belong to the
gpu-kernel-optimization skill.

## Layout questions — answer before `/data-oriented-design`

Which fields does the hot kernel touch (drives AoS vs SoA) · flat arrays vs linked structure ·
u32 indices vs pointers · dense vs sparse vs CSR/offset-plus-payload · fixed vs variable-size
records · partitioning across blocks · per-thread / per-warp / per-block / global storage
tiers · double buffering or ping-pong · pooled temporaries · explicit output-size computation
before the write.

Then invoke `/data-oriented-design` for the representation itself.

## Smells

Each of these is a design question, not automatically a bug — but each needs an answer:

per-object heap allocation · recursive pointer-heavy structures · virtual dispatch inside a
kernel · fine-grained host/device round trips · one shared global counter · unbounded per-thread
work · frequent grid-wide sync · one atomic per input item · loading a 64-byte record to read one
field · a CPU container memcpy'd to the device unchanged.

## Divergence and load balance

Warp divergence and predication · grouping work by type or expected cost · sorting/bucketing for
coherence · static vs dynamic assignment · warp-level work stealing · persistent work queues ·
frontier compaction · heavy-tailed task sizes · cooperative (warp- or block-wide) processing of
outsized tasks.

The standard fix for a heavy tail is not a bigger block — it is splitting the tail into a second
kernel with a different ownership level.

## Bottleneck classes

Classify each stage as limited by exactly one, then attack that one:

insufficient parallelism · memory bandwidth · memory latency · compute throughput · divergence ·
atomic contention · synchronization · kernel-launch overhead · host/device transfer · load
imbalance · register pressure · shared-memory capacity · excessive intermediate materialization.

Roofline analysis and the per-class attack playbook belong to the gpu-kernel-optimization
skill; here the job is only to name the class per stage.

## Benchmark plan

Measure: end-to-end including transfers · device-only · per-stage · throughput · effective
bandwidth · temporary memory · launch count · scaling with input size · scaling with batch size ·
break-even input size · deterministic vs nondeterministic mode.

Warm up, repeat, report the median (or state the statistic used). The speedup-claim checklist
lives in SKILL.md step 10.
