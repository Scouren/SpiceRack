# Example 1 — Prefix scan

```python
running = identity
for i in range(n):
    running = op(running, values[i])
    output[i] = running
```

**Why it looks sequential.** `running` is a single mutable variable carrying state across every
iteration. Edge class: true RAW on `running`, chain length n. Span n.

**Verdict:** artificial — the chain exists on `running` (one storage location), not on the
values. `output[i] = op(v[0], ..., v[i])`; nothing in that definition mentions an order.

**Enabling property.** `op` associative. Nothing else needed for correctness of the reassociation
— commutativity is *not* required, and assuming it is a common error that breaks scans over
matrix products, string concatenation, and min/max-plus segments.

**Blelloch up-sweep / down-sweep.** Build a reduction tree over the array (up-sweep), then push
partials back down assigning each node its exclusive prefix.

```
W = 2n adds,  S = 2 log n         (work-efficient)
```

**Hillis–Steele / naive doubling.** `x[i] += x[i - 2^k]` for k = 0..log n:

```
W = n log n adds,  S = log n      (span-efficient, work-inefficient)
```

**Choosing between them.** The log-n-span version does log n times the work. It wins only when
lanes are idle anyway — inside one warp or one block, where n is 32 or 256 and the shuffle-based
implementation avoids shared-memory traffic entirely. Across a large array it loses. Production
scans are therefore hierarchical: doubling within a warp, tree across the block, and a
single-pass **decoupled look-back** across blocks so the whole scan is one kernel and one pass
over memory rather than three.

**Floating point.** Reassociation changes the result. A tree scan over floats is not bitwise
equal to the sequential scan — usually it is *more* accurate, since the tree's error grows as
log n rather than n, but "more accurate" is still "different". If the contract says bitwise,
this transformation is illegal; say so and stop. If it says numeric within tolerance, record the
tolerance and pick `approx` in the harness.

**Mapping.** `cub::DeviceScan::ExclusiveScan` / `InclusiveScan`, `thrust::exclusive_scan`,
`rocPRIM` equivalents. Block-level: `cub::BlockScan`. Warp-level: `cub::WarpScan` or raw
`__shfl_up_sync`. A hand-written device-wide scan needs a measured reason; the library version
already runs at memory bandwidth.

**Beyond sums.** Once the operator is a parameter, scan absorbs a whole family: running max,
segmented scan (operator over (value, flag) pairs), run-length decoding, the offsets stage of
compaction and radix sort, and any lifted-state recurrence whose transformers compose
associatively.
