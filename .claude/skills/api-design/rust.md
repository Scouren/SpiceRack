# Rust API Patterns

How the five characteristics land in Rust. Reference for [`SKILL.md`](SKILL.md).

## Tagged unions over `dyn Trait`

A `dyn Trait` parameter is a vtable, and a vtable is a callback — the flow-control red flag in
disguise. It also scatters the per-variant code across impl blocks, hiding the shape that makes
batching possible.

```rust
// BAD: dispatch the caller cannot see through, one indirection per element,
// and no way to process all circles together.
fn total_area(shapes: &[Box<dyn Shape>]) -> f32 {
    shapes.iter().map(|s| s.area()).sum()
}

// GOOD: exhaustive, contiguous, and the structure is visible enough to split
// into per-variant passes when it gets hot.
enum Shape {
    Circle { radius: f32 },
    Rect { w: f32, h: f32 },
    Triangle { base: f32, height: f32 },
}

fn total_area(shapes: &[Shape]) -> f32 {
    shapes.iter().map(|s| match s {
        Shape::Circle { radius } => std::f32::consts::PI * radius * radius,
        Shape::Rect { w, h } => w * h,
        Shape::Triangle { base, height } => 0.5 * base * height,
    }).sum()
}
```

Keep `dyn` for the case it is actually for: a set of implementations that genuinely lives outside
your crate and that you cannot enumerate.

## Typestate instead of an init-order contract

Ordering coupling ("call `init()` first") is invisible in signatures and enforced by a panic.
Encode the dependency in a value instead, and the coupling becomes a declaration.

```rust
// BAD: nothing in the signature says this needs init(), and nothing stops you.
pub fn init() { ... }
pub fn create_surface() -> Surface { ... }   // panics if init() never ran

// GOOD: the Instance is the proof. The dependency is now in the type.
pub struct Instance { /* ... */ }
pub fn init(config: &Config) -> Result<Instance, Error>;
pub fn create_surface(instance: &Instance, desc: &SurfaceDesc) -> Result<Surface, Error>;
```

The same trick gates transitions, so the coarse tier is the only one that can skip a step:

```rust
pub struct Pipeline<S> { raw: RawPipeline, _state: PhantomData<S> }
pub struct Unbound;
pub struct Bound;

impl Pipeline<Unbound> {
    pub fn bind(self, target: &RenderTarget) -> Pipeline<Bound> { ... }
}
impl Pipeline<Bound> {
    pub fn draw(&self, mesh: &Mesh) { ... }   // Pipeline<Unbound> has no draw()
}
```

Do not reach for typestate on the fine tier. A caller who dropped to the raw command buffer is
there precisely because they want to sequence it themselves.

## Slices and primitives at the boundary

Take `&str` not `&String`, `&[T]` not `&Vec<T>`, `&[f32; 16]` not `&Mat4`. Every API-owned type in
a signature is a currying tax on someone who already has the data in another shape.

```rust
// BAD: the caller owns [f32; 3] and [f32; 4] and must build our type to ask a question.
pub fn transform_point(t: &Transform, p: &Point) -> Point;

// GOOD: diagonal doors onto the same code.
pub fn transform_point(t: &Transform, p: &Point) -> Point;
pub fn transform_point_raw(t: &Transform, p: [f32; 3]) -> [f32; 3];
```

Return owned types rather than lifetime-bound views where the caller would otherwise be fighting
the borrow checker at your boundary. A borrow that forces the caller to restructure their storage
is a coupling.

## Split allocation from initialization

The single biggest source of retention in a Rust library is a constructor that allocates.

```rust
// BAD: we own the allocation, so the caller cannot use an arena, a pool,
// or memory they mapped themselves.
pub fn decode(data: &[u8]) -> Result<Image, Error>;

// GOOD: three tiers. Most callers use the first; the ship-week caller uses the third.
pub fn decode(data: &[u8]) -> Result<Image, Error>;                       // convenience
pub fn decoded_size(header: &Header) -> usize;                            // query
pub fn decode_into(data: &[u8], dst: &mut [u8]) -> Result<ImageView, Error>;  // caller owns
```

`decode_into` writing into a caller-supplied `&mut [u8]` is the Rust spelling of "you provide the
memory, I initialize in place." It is worth more than an `Allocator` parameter, and it works on
stable.

## Iterators are the immediate-mode escape hatch

Returning a `Vec` is retention: you allocated, you own it, the caller synchronizes with it.
Returning an iterator, or writing into a caller slice, is immediate.

```rust
// BAD: allocates, and the caller cannot stream or stop early without paying for all of it.
pub fn find_matches(&self, pat: &str) -> Vec<Match>;

// GOOD: the caller decides whether it becomes a Vec, and when to stop.
pub fn matches<'a>(&'a self, pat: &'a str) -> impl Iterator<Item = Match> + 'a;
```

Rust's own guideline for this is **C-INTERMEDIATE** (expose intermediate results to avoid
duplicate work) and **C-CALLER-CONTROL** (caller decides where to copy and place data). Those two
items are the Rust Guidelines' encoding of the granularity and retention axes.

## Guidelines that carry these axes

From the Rust API Guidelines, the items that bear on systems design:

- **C-CALLER-CONTROL / C-INTERMEDIATE** — retention and granularity, as above.
- **C-CONV-TRAITS** (`From`, `AsRef`, `AsMut`) — the sanctioned way to add representation
  redundancy without writing N overloads.
- **C-NO-OUT** — return tuples rather than out-params. Rust's answer to the amalgamated grab-bag
  struct that made ETW unreadable.
- **C-CUSTOM-TYPE** — arguments convey meaning through types, not `bool`. A `bool` parameter is
  the "ClientContext = 1" magic-number failure.
- **C-SEND-SYNC** — a type that is neither is a hidden flow-control constraint.
- **C-COMMON-TRAITS** — eagerly derive `Debug`, `Clone`, `PartialEq`. A type the caller cannot
  print or compare is opaque data with no reason to be opaque.

Two of them cut the other way and you should know it: **C-STRUCT-PRIVATE** and **C-NEWTYPE-HIDE**
optimize for semver stability, which is directly opposed to "any data without a reason to be
opaque should be transparent." Resolve it deliberately — private fields plus accessors on the
retained tier, `#[repr(C)]` public plain-data structs on the fine tier, not private everywhere by
reflex.

## Zero-cost tiering with generics

Monomorphization gives you a coarse tier that compiles to the fine tier, so the convenience layer
is free:

```rust
// Fine tier: the caller drives the batch.
pub fn submit_draw(ctx: &mut Ctx, call: &DrawCall);

// Coarse tier: literally calls the fine one. Replaceable line for line.
pub fn draw_sprite(ctx: &mut Ctx, sprite: &Sprite, pos: Vec2) {
    submit_draw(ctx, &DrawCall::from_sprite(sprite, pos));
}
```

The test for a coarse tier is exactly this: can the caller delete the call and paste its body?
