# The shape in Zig

## The four steps

| Step | Zig |
|---|---|
| condition | ordinary comparison → `bool` (or `@Vector(n, bool)` on vectors) |
| widen to 0/1 | `@intFromBool(c)` → `u1`, then `@as(T, …)` |
| widen to a mask | `0 -% @as(usize, @intFromBool(c))` → `0` or all-ones |
| combine, arithmetic | `s += @as(u32, @intFromBool(p)) * x;` |
| combine, mask | `x & mask`, or the blend `b ^ ((a ^ b) & mask)` |
| combine, native | `@min`, `@max`, `@abs`, `@select(T, pred, a, b)` for vectors |
| clamp | `@min(@max(x, lo), hi)` |
| saturate | `+|`, `-|`, `*|`, `<<|` — `@as(u8, 255) +| 1 == 255` |
| wrap | `+%`, `-%`, `*%` |
| bit counting | `@popCount`, `@ctz`, `@clz` |
| tell the optimizer | `@branchHint(.unpredictable)` as the *first statement* in the branch |

`@abs` on a signed integer returns the **unsigned** type of the same bit width, so it cannot overflow on `minInt` the way a hand-rolled `(v + mask) ^ mask` does. `@min`/`@max` take 2+ args, work elementwise on vectors, and return the smallest/biggest *non-NaN*.

## The idioms

```zig
// conditional accumulate
s += @as(u32, @intFromBool(a[i] < 50)) * a[i];

// always store, conditionally advance — the compaction idiom
out[n] = x;
n += @intFromBool(x > threshold);

// mask smear, then blend
const mask: usize = 0 -% @as(usize, @intFromBool(c));
const r = b ^ ((a ^ b) & mask);

// ring buffer wraparound without a branch or a division
i = (i + 1) & (n - 1);          // n a power of two
i += 1; i -= n & (0 -% @as(usize, @intFromBool(i >= n)));   // general n

// vector form of the same shape
const m = x > @as(@Vector(8, i32), @splat(50));
const s = @reduce(.Add, @select(i32, m, x, @as(@Vector(8, i32), @splat(0))));
```

## Gotchas

- **There is no scalar `select_unpredictable`.** `@branchHint(.unpredictable)` lowers to LLVM `!unpredictable` metadata, which steers middle-end passes but *not* the x86 backend's cmov-conversion decision. For scalar code that must stay branchless, write the arithmetic/mask form and read the assembly.
- **`@branchHint` has placement rules.** First statement in a branch body or a function body only; it is silently ineffective after an `if` with a `noreturn` body and in a block-less switch prong.
- **The optimization barrier** stops LLVM from re-branching mask code it recognizes:
  ```zig
  mask = asm volatile ("" : [m] "=r" (-> usize), : [_] "0" (mask));
  ```
  Reach for it only after the assembly shows a jump you cannot remove otherwise — it also blocks legitimate optimizations across that point.
- **Float ordering** needs a self-inverse bit transform before comparators become branchless. `s ^ (mask & maxInt(SInt))` where `mask = s >> (bits - 1)` gives a total order `-NaN < -inf < … < +inf < +NaN` on the bitcast integers.
- **Safety-checked arithmetic is a branch.** In Debug and ReleaseSafe, `+`, `[]`, and `@intCast` all carry checks that show up as jumps. Compare assembly at the optimization mode you actually ship.
