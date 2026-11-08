# Example 3 — Wavefront dynamic programming

Edit distance / Smith–Waterman shape:

```python
for i in range(1, m):
    for j in range(1, n):
        D[i][j] = min(D[i-1][j] + 1,
                      D[i][j-1] + 1,
                      D[i-1][j-1] + cost(a[i], b[j]))
```

**Why rows cannot run independently.** `D[i][j]` needs its left neighbour, so a row is a chain of
length n. Rows need the row above, so columns are a chain of length m. Both edges are
mathematically required — this is not an implementation artifact.

**Verdict:** nested-parallel. The essential dependency is a partial order, not a total one:
`(i,j)` needs only cells with smaller i+j. All cells on one anti-diagonal are mutually
independent.

```
diagonal k = {(i, j) : i + j = k}
for k in 2 .. m+n:  process diagonal k in parallel
```

W unchanged at m·n. S = m + n instead of m·n. Parallelism (m·n)/(m+n) ≈ n/2 for a square — good
in the middle, zero at the corners. The first and last diagonals hold one cell each; a
diagonal-per-kernel-launch scheme spends launch overhead on work of size 1.

## Tiled wavefront — the version to actually write

Blocks of T×T cells. A tile needs its left tile's right column, its top tile's bottom row, and one
corner value. Tiles on a tile-diagonal are independent.

```
per tile: T² cells, computed sequentially-in-diagonals inside the tile in shared memory
tile grid: (m/T)·(n/T) tiles, tile-diagonal count (m+n)/T launches
```

Register/shared reuse goes from 0 to T (each loaded character serves T cells). Launch count drops
by T. This is the standard shape and the one the libraries use.

Inside a tile, keep the wavefront in registers and shift along the diagonal with
`__shfl_up_sync`: one thread per row of the tile, marching diagonally, no shared-memory round trip
per cell.

## Where the batch dimension rescues it

For one 1024×1024 alignment, peak parallelism is ~1024 lanes on a machine wanting ~10⁵ — most of
the GPU is idle at every wavefront, and the corners are near-serial. **Verdict: thin.**

For 100k short read alignments — the actual workload in bioinformatics — assign one alignment per
warp or per block and parallelize across the batch. Parallelism becomes 100k, divergence is nil
because every instance runs the same recurrence, and the wavefront inside each instance barely
matters. The right axis was never inside the DP.

## A transformation that is parallel and worse

Min-plus (tropical semiring) matrix products turn this recurrence into repeated squaring:
`S = O(log n)` instead of `O(m+n)`. Genuinely, provably shorter span.

It also turns `O(mn)` work into `O(n³ log n)`, materializes n×n intermediates that do not fit in
memory at realistic n, and pays full bandwidth for every one. At m=n=1024 that is roughly seven
orders of magnitude more arithmetic to remove a factor of ~100 from the span, on a machine that
has ~10⁵ lanes, not 10¹¹.

**Verdict: span-win-work-loss.** Keep it in the rejected table — for tiny n inside an inner loop
where the matrix fits in shared memory and the span dominates, it flips. That is why rejects stay
in the report.
