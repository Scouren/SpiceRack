# Measurement — profile first, optimize the bottleneck

## Profile-first workflow

Never optimize without a profile. Amdahl's Law caps the payoff: the serial/unoptimized
fraction bounds the speedup, so speeding up 5% of runtime by 10x saves 4.5% total.

| Fraction of runtime | 2x speedup | 10x speedup | Infinite speedup |
|---------------------|------------|-------------|------------------|
| 90%                 | 1.8x total | 5.3x total  | 10x total        |
| 50%                 | 1.3x total | 1.8x total  | 2x total         |
| 10%                 | 1.05x total| 1.1x total  | 1.1x total       |

1. Profile the real workload (not a microbenchmark)
2. Identify the top-1 hotspot
3. Classify it: compute-bound, memory-bound, or contention-bound?
4. Apply the matching fix from the decision tree below
5. Re-measure. Stop when the goal is met

## Decision tree

```
Is it slow?
├── No → Stop. Ship it.
└── Yes → Profile it.
    ├── Compute-bound (high IPC, low cache misses)
    │   ├── Reduce work: algorithmic improvement
    │   ├── SIMD: convert to SoA, align data → the simd-loops skill
    │   └── Branchless: eliminate unpredictable branches
    ├── Memory-bound (low IPC, high cache misses)
    │   ├── Shrink hot struct: split hot/cold fields
    │   ├── SoA layout: pack accessed fields contiguously
    │   ├── Sequential access: arrays > linked structures
    │   └── Prefetch: software hints for pointer-chasing
    └── Contention-bound (scales poorly with threads)
        ├── False sharing: pad atomics to cache line
        ├── Lock granularity: per-bucket > global
        └── Lock-free: atomic ops where contention is narrow
```

## Know the costs

| Access | Latency | Relative |
|--------|---------|----------|
| L1     | ~1 ns   | 1x       |
| L2     | ~3 ns   | 3x       |
| L3     | ~10 ns  | 10x      |
| RAM    | ~100 ns | 100x     |

- Keep hot data under 32-64 KB (L1). If the working set fits L1 you're compute-bound;
  if not, memory-bound.
- Sequential access gets hardware prefetch free; random access pays full latency. The
  prefetcher does NOT help pointer chasing, hash-table access, or strides > 2 KB —
  there, a software hint (`@prefetch` in Zig, `_mm_prefetch` in Rust nightly) placed
  2-4 iterations ahead can, but measure it.
- Align to 64-byte cache lines for SIMD and to avoid false sharing.

## Branch prediction

Predictable branches (always-true guards) are nearly free; data-dependent branches in
tight loops kill throughput (misprediction = 15-20 cycles of pipeline flush). Go
branchless only when the branch is in a tight inner loop, data-dependent, and
`perf stat -e branch-misses` confirms it; below ~1000 executions readability wins.
Sorting the data to make the branch predictable is the alternative.

```rust
// Branchy (bad if unpredictable):
fn abs_val(x: i32) -> i32 { if x < 0 { -x } else { x } }

// Branchless:
fn abs_val(x: i32) -> i32 {
    let mask = x >> 31; // arithmetic shift: all 1s if negative, all 0s if positive
    (x ^ mask) - mask
}
```

## False sharing

```rust
// BAD: counters on same cache line, threads thrash each other
struct Counters { a: AtomicU64, b: AtomicU64 } // likely same 64-byte line

// GOOD: pad to separate cache lines
#[repr(C, align(64))]
struct PaddedCounter(AtomicU64);
struct Counters { a: PaddedCounter, b: PaddedCounter }
```

## Caller owns the loop

The API rule that keeps hot paths optimizable: push data, don't pull. Never force the
user into a callback/visitor/framework.

```rust
// BAD: library owns the loop (pull/callback)
parser.on_token(|tok| { /* trapped inside library's control flow */ });

// GOOD: caller owns the loop (push/iterator)
for tok in parser.tokens(&input) {
    // caller decides control flow
}
```

## Anti-patterns

### Premature abstraction
```rust
// BAD: trait + impl for one type, "just in case"
trait Processor { fn process(&self, data: &[u8]) -> Vec<u8>; }
struct JsonProcessor;
impl Processor for JsonProcessor { /* only impl ever */ }

// GOOD: just a function until you have two implementations
fn process_json(data: &[u8]) -> Vec<u8> { /* ... */ }
```

### Iterator abuse in hot paths
```python
# BAD: allocates intermediate lists
result = list(map(transform, filter(predicate, big_list)))

# GOOD: single pass, no intermediates
result = [transform(x) for x in big_list if predicate(x)]
# BETTER if hot: pre-allocate and index loop (measure to confirm)
```

### Ignoring access pattern
```rust
// BAD: HashMap for 16 items (cache-hostile, hash overhead > scan)
let lookup: HashMap<u8, Value> = /* 16 entries */;

// GOOD: flat array or sorted Vec + binary_search for small N
let lookup: [(u8, Value); 16] = /* ... */; // fits in L1, linear scan wins
// switch to HashMap when N > ~64 and profile confirms
```
