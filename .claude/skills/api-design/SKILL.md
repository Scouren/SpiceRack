---
name: api-design
description: Design and evaluate system-level APIs against the five characteristics of reusable components. Use when designing or reviewing a library, engine, SDK, middleware, FFI boundary, or GPU/graphics wrapper in Rust, Zig, C, or Odin; when the user mentions integration discontinuity, API granularity, immediate vs retained mode, orthogonal vs diagonal, or getting rid of callbacks; or says "design an API for X" in a systems context.
---

# System-Level API Design

For libraries, engines, SDKs, and reusable components — not web/REST. Rooted in Casey Muratori's
_Designing and Evaluating Reusable Components_ (2004), the design that carried Granny 2 through
2600+ shipped SKUs.

## Rules one and two: write the usage code first

Do not open a header and start declaring types. Write the code a real caller would write, against
the API you wish existed. That draft is the specification.

The same move evaluates someone else's API: before reading their docs, integrate the imaginary
perfect version into your actual codebase. Then ask how close theirs comes. **Think in your terms
first, not theirs.** Casey diagnosed every flaw in Windows' ETW purely by writing down what the
API should have looked like, before touching MSDN.

## Integration discontinuity

The failure everything below defends against. Integration is not a task you do once — it recurs,
spiking when the project pushes hard and again at ship, when the hard constraints land.

At any moment the caller has a set of available integration options: each costs some work and
buys some capability, and they pick the cheapest one clearing the current bar. As the bar rises
they step upward through that set. A **discontinuity** is a gap in the set — the next thing they
need (streaming, threading, a custom allocator) has no cheap step, so they must jump to total
ownership or tear out your component entirely. Work goes _backwards_.

Discontinuities land at ship time, which is when they are least survivable. Users do not remember
that you saved them a week in month two; they remember the wall in the final month.

Encapsulation is how you build one. Every layer of wrapping and insulating removes options from
the set. Push it far enough and there is exactly one thing the caller can do with your API.

## The five characteristics

Casey found four; Chris Hecker told him "you have to talk about flow control, flow control is
implicit in all of this," making five.

These are **axes with trade-offs, not five things to maximize.** Only coupling and flow control
are one-sided. Optimizing the set does not mean "no retention, maximum granularity everywhere" —
it means every high tier has a path down to a low one.

### 1. Granularity — can I do A, or must I do ABC?

Splitting one call into smaller calls that do the same work with more caller control.

_Trade-off: flexibility vs simplicity._ Coarse means fewer calls and less to get wrong. Fine
means the caller can intervene.

The obvious reason to want fine grain is modifying the intermediate result. **The non-obvious
reason matters more: separating _when_ the two halves happen.** The caller may want the exact
same behavior, just with the halves on different threads, or the second half deferred to end of
frame. That is the reason games need granularity even when they never change the math.

```zig
// BAD: one atomic call. The caller cannot defer, thread, or inspect anything.
pub fn updateOrientation(node: *Node, dt: f32) void { ... }

// GOOD: the same work, three calls. Identical if called back to back;
// separable if the caller wants the delta computed on a worker thread.
pub fn getOrientation(node: *const Node) Quat;
pub fn getOrientationDelta(node: *const Node, dt: f32) Quat;
pub fn setOrientation(node: *Node, q: Quat) void;
```

The trap is a coarse call built on a **bundled type**. Drop `UpdateNode(node)` because you want
to change the update, and you inherit responsibility for the five things its render half also
did — work you never wanted to know about. The fix is cheap and almost nobody does it: offer the
same call taking the _parts_ of the node it actually uses, not the node.

```zig
// BAD: to escape either half, the caller must reimplement both.
pub fn updateAndRenderNode(node: *Node, dt: f32) void { ... }

// GOOD: same code path, unbundled parameters. Escaping update costs nothing extra.
pub fn updateNode(xform: *Transform, anim: *AnimState, dt: f32) void { ... }
pub fn renderNode(xform: *const Transform, mesh: *const Mesh, mat: *const Material) void { ... }
```

**Bar:** any function you would not call atomic when writing a real program should decompose into
two to four more granular ones (accessors don't count). Otherwise your user's last resort is
phoning you for a back door during their ship week.

### 2. Redundancy — can I do A, or B, to get the same result?

Two or more routes to one outcome.

_Trade-off: convenience vs orthogonality._ Redundant is convenient; orthogonal is one obvious way
to do things and less to hold in your head.

Chris Hecker, crediting Larry Wall via Jon Blow: **you want your API diagonal, not orthogonal.**
Going from one place to another, travel the shortest distance rather than the Manhattan path.
"Diagonal" is exactly this Redundancy axis — keep the axis-aligned primitives, then _add_ the
direct route on top.

vurtun states the ordering: **write the orthogonal API first**, then wrap combinations of
primitives into a diagonal one for ease of use. Then a user drops from high to low by replacing a
single call with its orthogonal counterpart.

Three forms worth building:

**Parameter redundancy — the anti-currying move.** If your call only accepts your own transform
type, every caller does a dance: pull position and rotation out of their struct, build yours,
call, pull the result back out, put it back in theirs. Offer the overloads.

```rust
// BAD: the only door is our type, so every call site curries in and back out.
pub fn invert_transform(t: &Transform) -> Transform;

// GOOD: diagonal shortcuts for the shapes callers actually hold.
pub fn invert_transform(t: &Transform) -> Transform;
pub fn invert_pos_rot(pos: [f32; 3], rot: [f32; 4]) -> ([f32; 3], [f32; 4]);
pub fn invert_mat4(m: &[f32; 16]) -> [f32; 16];
```

**Representation redundancy.** Accept the orientation as a quaternion _or_ a 3x3 matrix. Ship the
identity constant so nobody hand-rolls one.

**Bundling choice.** Given granular calls 1-2-3, a coarser tier can bundle 1+2 _or_ 2+3. Same
granularity, different cut. Offering both lets the caller choose which seam they keep.

### 3. Coupling — does doing A secretly require B?

A hidden link between two parts of the API that the caller cannot break.

_No trade-off._ Coupling only removes options. It is often unavoidable; it is never a feature.

The forms, each a real API someone shipped:

- **Inter-object.** `Simulate()` touches every object at once, so you cannot step one.
- **Hidden state.** `SetTime()` that every other call silently reads, creating an ordering
  contract nothing enforces.
- **Unnamed locks.** `Begin()`/`End()` with no handle identifying _which_ scope, so exactly one
  can be open and two subsystems can never both use it.
- **Shared internal buffers.** Returning a pointer to the same scratch buffer, so `string1` and
  `string2` are secretly the same memory.
- **Allocation welded to initialization.** The caller may want to supply the memory and have you
  init in place, or own the memory but init from their own packed stream. Split the two.
- **Your types for common data.** If your only orientation door is your matrix type, every caller
  converts constantly. Three vendors, three vector types, one miserable app.
- **Your file format.** If the only way to get an object is through your loader, they inherit your
  I/O and your format with no escape.

The last three are the ones that force the loading tier apart. The good decomposition is four
lines instead of one, and the caller controls all of it:

```zig
// BAD: reads the file, allocates, decompresses, and interprets, all welded together.
pub fn readMesh(path: []const u8) !*Mesh { ... }

// GOOD: caller owns the bytes, the allocation, and the timing of each stage.
pub fn meshSizeFor(header: *const MeshHeader) usize;
pub fn decompressMesh(dst: []u8, src: []const u8) !void;
pub fn meshFromMemory(mem: []u8) *Mesh;
```

And the tier below that: if the decompressed bytes have no reason to be opaque, publish the
layout so the caller can read the file their own way and never call you at all. `readMesh` is
still good and most users will call it. The mistake is having _only_ `readMesh`.

### 4. Retention — does the API hold state on my behalf?

Retention is when data you own must be mirrored into the API and kept in sync — a scene graph, a
parent link, a registered service, a cached `SetTime`.

_Trade-off: synchronization vs automation._ Retained buys automation and costs you the sync.

**The cost is that you end up writing the diff.** Your game knows what is true. The retained API
knows what it was told. So every frame you compute the difference between the two and hope you
got it right:

```rust
// BAD: retained. This is diff code, and it is where the bugs live.
if x_pressed && joint.is_none() {
    joint = Some(world.create_joint(rocket, pole));
} else if !x_pressed && joint.is_some() {
    world.delete_joint(joint.take().unwrap());
}
world.simulate(dt);

// GOOD: immediate. State your intent; the API figures out the rest.
if x_pressed {
    world.do_joint(rocket, pole);
}
world.simulate(dt);
```

Immediate mode lets you proceed **algorithmically** — code makes the decision and hands the result
straight to the API. A boolean on a retained joint is not a substitute: you would have to create
speculative joints between every rocket and every pole just to have something to toggle.

**Bar:** every retained construct has an immediate equivalent that takes the same data as
parameters. At the finest tier the API retains nothing you are responsible for. Caching behind
the scenes is fine — being made to do their caching for them is not.

### 5. Flow control — who calls whom?

_No trade-off._ If the application can stay in control, that is always simpler.

Think in stack shapes. You call the library and it returns: baseline. The library calls you back:
now the library is in the middle of your stack, you have lost your scope, and you are threading
`void*` context through to get it back.

**Inheriting from an API class is identical to registering a callback.** A virtual call is a
function pointer in a vtable. If you think those two are different, rethink it.

The alternative to callbacks is not "no I/O" — it is **inverting the request**. Instead of the
library calling you when it needs a file, it _yields_ what it needs and you resume it. vurtun's
coroutine API:

```zig
// BAD: the library owns your stack and your resource policy.
pub fn unzipExtract(path: []const u8, on_file: *const fn ([]u8) void) void { ... }

// GOOD: the library asks; the caller decides how and when to answer.
pub const Request = union(enum) {
    file_mapping: struct { offset: usize, len: usize },
    toc_memory: struct { size: usize, alignment: usize },
    output_memory: struct { size: usize },
};

pub fn unzip(state: *Unzip, req: *Request) bool { ... }

// while (unzip(&state, &req)) switch (req) { ... }
```

The caller owns every allocation and every mapping, can serialize the table of contents to disk
and reload it, and can extract in parallel by feeding one shared TOC to N independent states —
with zero changes inside the library. The library only ever knows about memory blocks.

**Red flag:** anything that _requires_ inheritance or a callback gives up flow control _and_
couples you. Every callback path needs a plain-call equivalent.

## The needs invert over a project's life

At first integration the caller wants **coarse granularity and lots of retention** — load some
characters, animate them, free them. By ship they want the reverse: fine granularity in the few
places they must manhandle, and as little retention as possible, because by then they have built
the data structures that encode how their program actually works and every mirrored copy is a
liability.

This is why tiers, not levels, are the deliverable. The same user needs opposite ends of your API
at different times, and gets there by stepping.

## Semantic compression

Do not design the architecture first. **Make your code usable before you try to make it reusable.**

1. Type out exactly what you want to happen in each specific case, with no regard for correctness
   or abstraction, and get it working.
2. Do not reuse anything until you have **two** instances of it. One example (or zero, for code
   written preemptively) is not enough information to factor correctly, and you will end up with
   something that isn't conveniently reusable.
3. On the second instance, pull out the shared portion — _compress_ it. Name it for what it means
   in the problem's own language.
4. On the third and later uses, decide: use it as-is, change how it works, or add a layer above or
   below it. That last option is where tiers come from.

Repeat. The architecture is the residue.

The reason this beats planning: **the hard part of code is getting the details right.** Starting
where the details don't exist guarantees you overlook something that invalidates the plan.
Compressing from working details cannot make that mistake. Objects created this way are exactly
the ones you wanted, and they were trivial to design.

Compressed code is simultaneously minimal (there is little of it), consistent (identical
behaviors share one path), and extensible (the pieces are already separable).

## Language patterns

- Rust: `rust.md` — typestate, tagged unions over `dyn`, slices at the boundary, the guidelines
  that encode these axes.
- Zig: `zig.md` — allocator parameters, comptime tiers, error sets, `defer`/`errdefer`, the
  request-union loop in full.

## Evaluation checklist

Answer all of these against real usage code, not against the header.

1. Can you write the integration in under ten minutes, having written it your way first?
2. Does every high-level call decompose into two to four lower-level ones?
3. Is the common case reachable diagonally, in one or two calls?
4. Can you use feature A without initializing unrelated feature B?
5. Does every retained construct have an immediate-mode twin taking the same data as parameters?
6. Does the caller decide when everything happens — no required callbacks, no required inheritance?
7. Can you pass your own types and read your own files, or must you use theirs?
8. Is data that has no reason to be opaque actually transparent?
9. Is allocation separable from initialization?
10. Can you get the source to the runtime?
11. Where is the discontinuity? Name the requirement that would force a caller to jump or tear out.

## Sources

- Casey Muratori, _Designing and Evaluating Reusable Components_ (2004) — caseymuratori.com/blog_0024
- Casey Muratori, _The Worst API Ever Made_ — caseymuratori.com/blog_0025
- Casey Muratori, _Semantic Compression_ — caseymuratori.com/blog_0015
- Chris Hecker, _API Design_ — chrishecker.com/API_Design
- vurtun, _API Design: Coroutine APIs_ — gist.github.com/vurtun/192cac1f1818417d7b4067d60e4fe921
- Rust API Guidelines — rust-lang.github.io/api-guidelines/checklist.html
