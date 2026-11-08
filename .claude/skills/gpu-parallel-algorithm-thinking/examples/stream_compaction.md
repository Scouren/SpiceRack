# Example 2 — Stream compaction

```python
output = []
for item in items:
    if predicate(item):
        output.append(transform(item))
```

**Why it looks sequential.** `append` reads and writes a hidden output cursor. Every surviving
item depends on the cursor value left by the previous survivor: RAW chain of length n.

**Verdict:** artificial. Edge class: counter-or-allocator. The cursor is not part of the
specification — the specification says "survivors, in input order". Each survivor's slot is a
*function* of the input: the number of survivors before it. That is an exclusive scan of the
predicate flags.

## Count–scan–scatter

```
flags[i]   = predicate(items[i])            map,           W=n  S=1
offset[i]  = exclusive_scan(flags)[i]       scan,          W=n  S=log n
total      = reduce(flags)                  fused into the scan
if flags[i]: out[offset[i]] = f(items[i])   scatter,       W=n  S=1
```

Three stages, W = O(n), S = O(log n), one extra n-element buffer (or zero, if flags are
recomputed in the scatter — recompute instead of communicate, when `predicate` is cheap).
Deterministic: every element's destination is a pure function of the input.

Runnable harness for exactly this: `scripts/example_compaction.py`.

## The atomic alternative

```
slot = atomicAdd(&cursor, 1);  out[slot] = f(item);
```

Correct — every survivor gets a distinct slot, no slot is skipped. Two costs the scan version
does not pay:

1. **Contention.** One atomic per surviving input against a single address. Fine at low survival
   rates; at high rates the atomic unit serializes and becomes the bottleneck.
2. **Nondeterminism of order.** Output order depends on scheduling. If the contract requires
   input order, this is wrong, not just slow — and it will pass a naive test suite that compares
   with `unordered`. This is precisely why the comparator mode has to be chosen from the contract
   in step 1 rather than from whatever makes the suite green.

**The middle path, and usually the fastest:** aggregate per warp with a ballot plus a warp scan,
then one `atomicAdd` per warp for the whole group. Contention drops by up to the warp width;
order within the warp is preserved, order across warps is not. `cub::WarpScan` plus
`__ballot_sync`, or just let CUB do it.

**Mapping.** `cub::DeviceSelect::If` / `Flagged`, `thrust::copy_if`, `rocPRIM::select`. Fusing
`transform` into the predicate pass avoids materializing an intermediate array — the transform is
free in a bandwidth-bound stage.
