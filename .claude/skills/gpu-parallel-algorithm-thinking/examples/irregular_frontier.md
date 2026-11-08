# Example 4 — Irregular frontier (BFS)

```python
queue = deque([src]); dist = {src: 0}
while queue:
    u = queue.popleft()
    for v in adj[u]:
        if v not in dist:
            dist[v] = dist[u] + 1
            queue.append(v)
```

**Why it looks sequential.** A single FIFO, a single visited map, one vertex at a time. Edge
classes: counter-or-allocator on the queue tail, data-structure-induced on the hash map,
control on the `if`.

**Verdict:** artificial, with a residual essential depth. The queue's FIFO discipline is
over-specification — BFS only requires that level k be finished before level k+1 starts. Level
membership is the real structure; the order *within* a level is free. Essential span = the graph's
eccentricity from the source (diameter-ish), which is small for social graphs and large for road
networks. That single number decides whether this belongs on a GPU at all.

## Bulk-synchronous levels

```
frontier
 → expand: each frontier vertex reads its neighbours          (irregular, load-imbalanced)
 → count generated work per vertex = degree
 → exclusive scan the counts                                  (per-vertex output offsets)
 → write next frontier into the scanned slots                 (independent writes)
 → dedupe / compact
 → repeat until the frontier is empty
```

One kernel (or three) per level, `O(diameter)` global synchronizations, W = O(V + E) total.
Count–scan–scatter again — the same move as compaction, applied to generated work instead of
survivors.

## Load imbalance is the whole problem

Degrees are heavy-tailed. One vertex per thread means one thread in a warp doing 10⁵ edges while
31 threads idle: the warp runs at 1/32 efficiency for the duration of the tail.

Fixes, in increasing order of effort:

- **Per-thread / per-warp / per-block dispatch by degree.** Small degrees → one thread. Medium →
  the whole warp cooperates on one vertex. Large → the whole block. Three code paths, chosen by a
  degree threshold; each path is internally uniform, so divergence disappears.
- **Load-balanced search.** Scan the degrees, then binary-search the scanned offsets to map a flat
  edge index to its owning vertex. Every thread gets exactly one edge. Perfect balance, at the
  cost of a search per thread and losing per-vertex register reuse. `cub::DeviceSegmentedReduce`
  and merge-path partitioning use the same trick.
- **Persistent blocks with a work queue.** Blocks pull chunks until the frontier drains. Best for
  extreme skew, worst for complexity and for determinism.

## Duplicates

Multiple frontier vertices discover the same neighbour in the same level. Three ways out:

- **Atomic visited bitmap.** `atomicOr` / `atomicCAS` per discovered vertex. Cheap and correct;
  `dist` is idempotent so races are benign. Contention concentrates on high-degree hubs.
- **Sort–unique the next frontier.** Deterministic, no atomics, but a full radix sort per level —
  usually more expensive than the atomics it replaces. **Parallel and worse** on most graphs; keep
  it for the case where determinism is contractual.
- **Tolerate them.** Because relaxation is idempotent, duplicate work is correct, just wasted.
  Often the cheapest option for small duplicate counts.

Note the pattern: idempotence is what makes the cheap options legal. That property was found in
step 4, not in the kernel.

## Push vs pull

**Push** (expand from the frontier, write to neighbours) costs work proportional to frontier
degree — good while the frontier is small. **Pull** (every unvisited vertex checks whether any
neighbour is in the frontier) costs work proportional to unvisited count — good when the frontier
is huge, and it needs no atomics because each vertex writes only its own slot.

**Direction-optimizing BFS** switches between them per level on a frontier-size heuristic, and
switches the frontier representation with it: sparse (a compacted index array) while small, dense
(a bitmap over all V) while large. On a social graph the middle levels touch most of the graph and
pull wins by a large factor; on a road network the frontier never gets big and push wins
throughout.

**Mapping.** CSR (row offsets + column indices) — the offset-plus-payload layout, and exactly
what `/data-oriented-design` produces from an adjacency-list-of-pointers. Gunrock and
`nvGRAPH`/cuGraph implement the whole thing; reach for them before writing level kernels.

**When to walk away.** A path graph has diameter V: V global synchronizations, one vertex of work
each. **Verdict: thin** — the CPU finishes before the GPU has finished launching. High-diameter
graphs (meshes, roads, chains) are a real, common case where the honest answer is CPU or hybrid.
