# Foundations: Data-Oriented Design, Negative-Overhead Abstraction, and API Design

The theory document, language-agnostic; the companions (rust.md, zig.md) show how to
realize each idea. The three bodies of work here are usually presented as rivals. They
are not — all three converge on one posture: **transparent data, primitives first,
abstraction discovered by compression, decisions verified by measurement.**

---

## Part I — Data-Oriented Design (Mike Acton, Andrew Kelley)

### The thesis

> The purpose of all programs, and all parts of those programs, is to transform data
> from one form to another.

This is Acton's first principle, and it is the hinge for everything else. It is not a
performance claim; it is an ontological one. A function, a module, and a whole service
are all, at bottom, a mapping from input bytes to output bytes. If you describe a unit
of code and cannot state what data goes in and what data comes out, you do not yet
understand it. (Brooks said the same thing decades earlier: _show me your tables and I
won't usually need your flowcharts._)

### Acton's principles, in full

The epistemics — how you come to understand a problem:

- The purpose of all programs is to transform data. (P1)
- If you don't understand the data, you don't understand the problem. (P2)
- Conversely, understand the problem by understanding the data. (P3)
- Different problems require different solutions. (P4)
- If you have different data, you have a different problem. (P5)
- Everything is a data problem — including usability, maintenance, debuggability. (P8)
- Solving problems you probably don't have creates problems you definitely do. (P9)

The cost model — you cannot reason about a solution without reasoning about the machine:

- If you don't understand the cost of solving the problem, you don't understand the
  problem. (P6)
- If you don't understand the hardware, you can't reason about the cost. (P7)
- Latency and throughput are the same only in sequential systems. (P10)
- Software does not run in a magic aether; it runs on hardware. (P11)

The rules of thumb:

- **Where there is one, there are many.** Look on the time axis. You will almost never
  process exactly one of a thing; design for the collection. (ROT1)
- The more context you have, the better the solution. Don't throw away data you need.
  (ROT2)
- Non-uniform access (NUMA) extends backward through I/O and pre-built data all the way
  to original source creation. (ROT3)

### The three lies

Acton names three industry assumptions that pull people away from the data:

1. **Software is a platform.** The hardware is the platform. The cache hierarchy, the
   line size, the memory latency — those are the ground truth your abstractions sit on.
2. **Code should be designed around a model of the world.** Modeling the world via
   `IS-A` hierarchies confuses what is easy to _say_ in English with what is efficient
   to _compute_. A `Cat IS-A Animal` taxonomy tells you nothing about how to lay bytes
   out for the transformation you actually run.
3. **Code is more important than data.** If the purpose of code is to transform data,
   then data is primary and code is the mechanism.

### The hardware reality you are designing against

A modern CPU core can execute on the order of hundreds of instructions in the time it
takes to service a single main-memory cache miss. The dominant cost in most data-heavy
loops is therefore not arithmetic — it is waiting for memory. Two consequences:

- Data you touch together should live together (spatial locality). A cache line is
  fetched whole; if half of it is fields you don't read this pass, you've wasted half
  your bandwidth.
- Predictable, linear access lets the hardware prefetcher and the compiler's
  vectorizer work. Pointer-chasing and unpredictable branches defeat both.

### Kelley's practical playbook

Andrew Kelley applied DOD to the Zig compiler and reduced its memory footprint and
cache pressure substantially. His summary is "use less memory, go fast." The concrete
moves, each of which has a direct realization in both companion documents:

1. **Indices instead of pointers.** A pointer is 8 bytes and pins lifetime/aliasing. An
   index into an array is often a `u32` (or smaller), halving the footprint and letting
   you relocate the backing storage freely. Pointers become positions in an arena.
2. **Store booleans (and other rare fields) out of band.** A single `bool` in a struct
   frequently costs far more than one bit once padding is accounted for. Hot fields stay
   in the main array; rare/cold fields move to a side table keyed by index.
3. **Eliminate padding with struct-of-arrays.** Reordering fields removes some padding;
   splitting a struct into one array per field (SoA) removes inter-field padding
   entirely and lets a pass over a single field stream contiguously.
4. **Encodings instead of polymorphism.** Rather than a vtable per element, store a
   small tag (an enum) plus a compact payload, and branch on the tag at the top of the
   loop — or better, bucket by tag so each loop is monomorphic.
5. **Shrink the element.** Smaller structs mean more elements per cache line, which
   often unlocks autovectorization for free.

### Array-of-Structs vs Struct-of-Arrays, stated precisely

```
AoS:  [ {pos,vel,color,flags}, {pos,vel,color,flags}, ... ]   // one element contiguous
SoA:  pos[]  vel[]  color[]  flags[]                          // one field contiguous
```

- **SoA wins** when a pass reads a _subset_ of fields across _many_ elements
  (integrate position from velocity; sum one column). It minimizes cache-line waste and
  vectorizes. This is the same reason analytical databases (OLAP) are columnar.
- **AoS wins** when a pass reads _most/all_ fields of _one_ element at a time
  (serialize a record; an update where the fields are mutually dependent), because the
  whole element is in one line.

The rule is Acton's, not a slogan: **look at the data, look at the process, make the
layout fit the access pattern.** SoA is not "better"; it is better _for column-wise
access_.

### DOD as a design method, not just an optimization

Because every module is "a large function that transforms input data into output
data," DOD gives you a decomposition rule: **group data that is used together; separate
data that is not.** Module boundaries fall where data is stable enough to form an API.
This is a design discipline that pays off in modifiability and testability even when
performance is not the goal — the data flow _is_ the architecture.

---

## Part II — Negative-Overhead Abstraction ("Clean Code," reconsidered)

### The false dichotomy

The usual framing pits "clean, abstracted code" against "fast code." That framing is
the error. The right question is never _whether_ to abstract but _whether the optimizer
can see through the abstraction you chose._ When it can, the abstraction is **zero-cost**
(erased entirely) or **negative-cost** (it compiles to something better than the obvious
hand-written version).

### Definitions

- **Zero-cost (Stroustrup):** what you don't use, you don't pay for; and what you do
  use, you couldn't reasonably hand-code any better.
- **Operational test:** a zero-cost abstraction compiles to the best implementation a
  competent programmer would have written using the low-level primitives directly. It
  must not introduce costs that could be avoided by _not_ using it.
- **Negative-cost:** the abstraction carries an invariant the compiler can exploit
  (e.g. an iterator knows its length, so per-element bounds checks are elided and the
  loop vectorizes), producing machine code _faster_ than naive primitive-level code a
  person would typically write.

### The data/object anti-symmetry (Clean Code, Ch. 6)

The one directly data-oriented idea in Martin's book, usually overlooked. He
distinguishes

- **objects**, which hide their data behind abstractions and expose behavior, from
- **data structures**, which expose their data and have essentially no behavior,

and observes the trade-off: procedural code over _data structures_ makes it easy to add
new operations without touching the structures, while object-oriented code makes it
easy to add new types without touching existing operations. DOD lives almost entirely
in the "data structure" quadrant: transparent data, with transformations written as
free functions over it. Don't grow behavior on the things that are really just data.

### Where "Clean Code" misleads if applied as dogma

The prescriptions that draw fire are the ones that fight either the optimizer or the
data:

- **"Functions must be very small."** Past the point of cohesion, fragmentation hides
  the data flow and scatters a transformation across call sites, making it harder for
  both the reader and the optimizer to see the whole loop. Size should follow one
  coherent transformation, not a target line count.
- **"Prefer polymorphism to conditionals."** A `match`/`switch` over a small closed set
  is usually clearer _and_ faster than dynamic dispatch; virtual dispatch in a hot loop
  is a vtable indirection per element that blocks inlining and vectorization. Reach for
  runtime polymorphism only when the set of cases is genuinely open at runtime — and
  even then, prefer _bucketing by type_ (DOD) over per-element dispatch.
- **"Encapsulate everything."** Reflexive getters/setters around fields with no
  invariant add friction and zero safety, and they make the SoA/transparent-data
  transformation harder.

### The discipline that keeps this honest

Zero-cost is a goal, not a guarantee. A newtype can occasionally defeat
autovectorization; dynamic dispatch and atomic reference counting introduce real
indirection; a missed inline turns a "free" combinator into a function call. Therefore:
**on any path that matters, verify** — read the generated assembly (e.g. on godbolt) or
benchmark the abstraction against the primitive. Claims of zero cost are checked, not
assumed.

---

## Part III — API Design (Casey Muratori)

### Method: write the usage code first

Before implementing anything, write the code you _wish_ you could call. The API is then
defined by the call site, not by the implementer's mental model. This is the single
highest-leverage practice, and it composes with compression (below): you write concrete
usage, then compress.

```rust
// STEP 1: Write the usage you want
let img = load_png("input.png");
let gray = to_grayscale(&img);
let edges = detect_edges(&gray, 1.4);
save_png(&edges, "output.png");

// STEP 2: Only now implement load_png, to_grayscale, etc.
// STEP 3: If detect_edges and to_grayscale share a per-pixel loop, compress into one
```

### The five characteristics (the evaluation framework)

For any component boundary between caller A and component B, evaluate:

1. **Granularity — "A or BC."** Can an operation be split into smaller steps that give
   the caller more control? An API offered only at coarse grain traps callers who need
   to intervene in the middle.
2. **Redundancy — "A or B."** Are there multiple paths to the same result, and is the
   _common_ path short? (This is the "diagonal, not orthogonal" idea — for a frequent
   operation you want the direct route, not a forced Manhattan walk through primitives.)
3. **Coupling — "A implies B."** Does using one part force you to take on another
   (a type, an allocator, a subsystem)? Minimize implied dependencies.
4. **Retention — "A mirrors B."** Does the component force the caller to mirror or hold
   state/data structures on its behalf? Forced retention couples the caller's data
   layout to the library's — anathema to DOD.
5. **Flow control — "A invokes B."** Who calls whom? A library that calls _you_ (owns
   the loop, the lifecycle) is far more invasive than one you call.

### The design rules that follow

- **Every non-atomic function should be expressible via two to four more granular
  functions.** Provide the coarse convenience call, but build it on the fine-grained
  ones and expose those too. Never trap the caller in only the high-level path.
- **Data that has no clear reason to be opaque should be transparent** — in
  construction, access, and I/O. (Directly DOD-compatible: transparent data can be laid
  out SoA, serialized directly, inspected.)
- **Use of the component's resource management is optional.** The caller can supply
  memory/files/strings. (Directly DOD-compatible: caller-owned arenas.)
- **Use of the component's file format is optional;** full runtime source is available.

### Memory and lifetime belong to the caller

Take an allocator / output buffer as a parameter instead of allocating behind the
caller's back. This is the same discipline Zig enforces by passing `Allocator`
everywhere and Rust enforces through ownership — and it is what makes a component usable
inside an arena, on the stack, or in a hot loop without forking it.

### Build by compression, not speculation (semantic compression)

The architecture should _emerge_. Write the concrete code first; then act like a
compressor on your own source — factor out the patterns that have genuinely repeated,
and only those. If a pattern appears once, inline it. Twice, watch it. Three times,
compress. A little duplication is cheaper than the wrong abstraction; two blocks that
merely look alike but evolve differently were never the same thing. Abstractions
discovered by compression are tight, used in many places, and map to real structure —
which is exactly the kind of abstraction a compiler also optimizes well.

---

The operational realizations — memory and allocation, concurrency, type-driven design,
SIMD — live in the companions: rust.md and zig.md §6–§9.
