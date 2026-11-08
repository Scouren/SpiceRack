# Data-Oriented Design — Reference

## Anti-Patterns

### 1. Array of Structs with cold fields polluting cache lines

**BAD** — every iteration loads `name` and `created_at` into cache, wastes bandwidth:
```rust
struct Entity {
    position: Vec3,
    velocity: Vec3,
    name: String,        // cold: only used in debug UI
    created_at: Instant,  // cold: only used in analytics
}

fn update_positions(entities: &mut [Entity]) {
    for e in entities { e.position += e.velocity; }
    // Each cache line wasted on name + created_at bytes
}
```

**GOOD** — hot/cold split, SoA:
```rust
struct Positions(Vec<Vec3>);
struct Velocities(Vec<Vec3>);

// Cold data in separate table, same index
struct EntityMeta { name: String, created_at: Instant }
struct MetaTable(Vec<EntityMeta>);

fn update_positions(pos: &mut Positions, vel: &Velocities) {
    for (p, v) in pos.0.iter_mut().zip(&vel.0) { *p += *v; }
    // Cache lines contain ONLY position and velocity data
}
```

### 2. Existence via boolean flags

**BAD** — branching on every element, wasting iteration on inactive entities:
```rust
struct Bullet {
    active: bool,
    position: Vec3,
    velocity: Vec3,
}

fn update_bullets(bullets: &mut [Bullet]) {
    for b in bullets {
        if !b.active { continue; }  // branch per element, sparse iteration
        b.position += b.velocity;
    }
}
```

**GOOD** — existence-based, swap-remove dead bullets:
```rust
struct Bullets {
    positions: Vec<Vec3>,
    velocities: Vec<Vec3>,
}

impl Bullets {
    fn kill(&mut self, i: usize) {
        self.positions.swap_remove(i);
        self.velocities.swap_remove(i);
    }
}

fn update_bullets(b: &mut Bullets) {
    for (p, v) in b.positions.iter_mut().zip(&b.velocities) {
        *p += *v;  // no branches, dense, SIMD-friendly
    }
}
```

### 3. Pointer-heavy graph via Box/Rc

**BAD** — cache-hostile pointer chasing:
```rust
struct Node {
    data: f32,
    children: Vec<Box<Node>>,  // each child is a separate heap allocation
}
```

**GOOD** — arena with indices:
```rust
struct NodeId(u32);
struct Arena {
    data: Vec<f32>,
    children: Vec<Vec<NodeId>>,  // indices, not pointers
}
// All nodes contiguous in memory. Tree traversal = index lookups into flat arrays.
```

### 4. Virtual dispatch / trait objects for homogeneous work

**BAD** — dynamic dispatch per element:
```rust
trait Updatable { fn update(&mut self); }
fn update_all(objects: &mut [Box<dyn Updatable>]) {
    for obj in objects { obj.update(); } // vtable lookup per element, no SIMD
}
```

**GOOD** — separate arrays per concrete type, process each in bulk:
```rust
fn update_all(players: &mut Players, bullets: &mut Bullets, enemies: &mut Enemies) {
    update_players(players);   // tight loop, one type
    update_bullets(bullets);   // tight loop, one type
    update_enemies(enemies);   // tight loop, one type
}
```

### 5. HashMap where a dense array works

**BAD** — hash overhead when IDs are sequential:
```python
entities = {}
for i in range(10000):
    entities[i] = {"x": 0.0, "y": 0.0}  # hash per lookup, scattered memory

for eid, e in entities.items():
    e["x"] += 1.0  # dict overhead per access
```

**GOOD** — flat arrays with index = ID:
```python
xs = [0.0] * 10000
ys = [0.0] * 10000

for i in range(len(xs)):
    xs[i] += 1.0  # contiguous, vectorizable
```

## Quick Reference — Layout Decision Table

| Question | Answer | Layout |
|---|---|---|
| Always access all fields together? | Yes | AoS |
| Iterate one or two fields at a time? | Yes | SoA |
| Field accessed < 10% of iterations? | Yes | Cold split |
| Need ordered lookup? | Yes | Sorted array or B-tree |
| Need fast membership test? | Yes | Bitset or dense set |
| IDs are sequential integers? | Yes | Flat array, index = ID |
| IDs are sparse / external? | Yes | Sparse-to-dense map |
| Elements created/destroyed often? | Yes | Swap-remove + generation index |
| Need pointer-like handle? | Yes | Index + generation counter |
| Polymorphic processing? | Yes | Separate array per type, no vtable |

## Zig-Specific Patterns

**Comptime SoA generation** — use `std.MultiArrayList` instead of hand-rolling:
```zig
const Entities = std.MultiArrayList(struct {
    position: Vec3,
    velocity: Vec3,
    health: u16,
});
// Automatically stores as SoA. Access: entities.items(.position)
```

**Explicit allocators** — pass allocators, never use a global heap:
```zig
fn createBullets(allocator: std.mem.Allocator, count: usize) !Bullets {
    return .{
        .positions = try allocator.alloc(Vec3, count),
        .velocities = try allocator.alloc(Vec3, count),
    };
}
```

**Arena pattern** — use `std.heap.ArenaAllocator` for bulk lifetime:
```zig
var arena = std.heap.ArenaAllocator.init(std.heap.page_allocator);
defer arena.deinit(); // frees everything at once
const alloc = arena.allocator();
```

**SIMD-friendly iteration** — vectorizing a hot loop is the simd-loops skill's job; its [zig.md](../simd-loops/zig.md) carries the `@Vector` idioms.

## Data Normalization (In-Memory Relational Thinking)

Apply the same normalization principles as relational DB design:
- **No duplicate data.** One source of truth per fact. If two systems need the same data, they reference it by index/ID.
- **Separate by update frequency.** Data that changes every frame and data that changes on user input belong in different tables.
- **Foreign keys = indices.** An enemy's weapon is `weapon_id: u32` into a weapons table, not an embedded weapon struct.
