# Decision records and change drills

Three artifacts come out of steps 6 and 7: one **seam record** per seam, one **drill table** per design or feature, and `ARCHITECTURE.md`. Keep the records in the project's decision log (`docs/dev/decisions.md`, `ARCHITECTURE.md` § Seams, or an ADR folder) where the code review can find them.

## Seam record

One per seam. Every field is filled; "n/a" is allowed only where the field says so.

```markdown
### S<n>: <what varies, in a few words>

- **Axis**: <what varies: kind | backend | platform | dialect | policy | plugin> — <the cases>
- **Evidence**: two cases today: <A>, <B> | drill <D#> needs it
- **Owner**: <module that owns the seam's tables> (<file>)
- **Rung**: <#> <mechanism>. Rung <#-1> fails because <reason tied to the axis>.
- **Cost**:
  - residue: <none | where it sits and how many per run: "1 indirect call per device kind per Newton step">
  - change radius for one more case: <n bodies> (<what gets added>)
  - interface weight: <the concepts a caller learns>
- **Drills passed**: <D#, D#>
- **Proof** (rung ≥5; n/a below): <verdict> — <probe file>, <toolchain + flags>
- **Revisit when**: <the observable event that would move it a rung: "a third backend", "kinds loaded at runtime">
```

## Drill table

One row per likely change. The changes come from the user, or from the backlog with the source cited; never invented.

```markdown
| ID | Change | Source | Edits (file: function or row) | Radius | Weight Δ | Verdict | Fix |
|---|---|---|---|---|---|---|---|
| D1 | <change> | <user / issue #> | <every edit> | <n bodies> | <+n concepts> | pass / fail | <restructure: … / climb S<n> to rung #> |
```

**Radius** counts existing function bodies edited. New files, new functions, new rows, new variants, and new spine lines are additions and count as 0. For a new enum variant, count the switch sites the compiler lists and say so ("3 arms, compiler-listed"). **Weight Δ** counts concepts a caller of an existing module must newly learn; a change that adds a module with its own interface counts only what existing callers see.

A failed drill is fixed by restructuring first (split the transform so the new behaviour runs as a new transform after it), and by climbing one rung only if restructuring cannot bring the radius down. Re-run the drill after the fix and record the new row.

## Worked example

Zig 0.17; the test passes as written (`zig test`). An object-shaped module:

```zig
const Entity = struct {
    vtable: *const VTable, // update() differs per kind
    pos_y: f32,
    vel_y: f32,
    on_ground: bool, // flag checked inside every update
    name: []const u8, // cold field in the hot line
};

pub fn step(entities: []*Entity, dt: f32) void {
    for (entities) |e| e.vtable.update(e, dt); // what happens in a frame? read every update()
}
```

The frame's behaviour is scattered across N `update` bodies, and "everything bounces" is an edit to every one. Steps 1 to 5 give: data `bodies` + `ground_hits`; one owner (`sim.zig`); one spine; one seam (kind), whose cases differ only in numbers, so rung 3 (value tables) instead of the vtable's rung 8.

```zig
const std = @import("std");
const assert = std.debug.assert;

pub const Kind = enum(u8) { rock, balloon };

// Seam S1, rung 3: kinds differ only in numbers. One row per kind, no switch anywhere.
const gravity_scale = std.EnumArray(Kind, f32).init(.{ .rock = 1.0, .balloon = -0.2 });
const bounce = std.EnumArray(Kind, f32).init(.{ .rock = 0.3, .balloon = 0.9 });

const Body = struct { pos_y_m: f32, vel_y_ms: f32, kind: Kind };
const Hit = struct { body: u32 };

pub const World = struct {
    bodies: std.MultiArrayList(Body) = .empty, // owner: sim
    ground_hits: std.ArrayList(Hit) = .empty, // owner: sim (request table, cleared per frame)
    bodies_max: u32,
};

// ---- spine: the whole frame, top to bottom ----
pub fn step(world: *World, dt_s: f32) void {
    const s = world.bodies.slice();
    applyGravity(s.items(.vel_y_ms), s.items(.kind), dt_s);
    integrateBodies(s.items(.pos_y_m), s.items(.vel_y_ms), dt_s);
    world.ground_hits.clearRetainingCapacity();
    collectGroundHits(s.items(.pos_y_m), &world.ground_hits);
    resolveGroundHits(world.ground_hits.items, s.items(.pos_y_m), s.items(.vel_y_ms), s.items(.kind));
}

// ---- transforms, in spine order ----
fn applyGravity(vel_y_ms: []f32, kind: []const Kind, dt_s: f32) void {
    assert(vel_y_ms.len == kind.len);
    for (vel_y_ms, kind) |*v, k| v.* -= 9.81 * gravity_scale.get(k) * dt_s;
}

fn integrateBodies(pos_y_m: []f32, vel_y_ms: []const f32, dt_s: f32) void {
    assert(pos_y_m.len == vel_y_ms.len);
    assert(dt_s > 0);
    for (pos_y_m, vel_y_ms) |*p, v| p.* += v * dt_s;
}

fn collectGroundHits(pos_y_m: []const f32, hits: *std.ArrayList(Hit)) void {
    assert(hits.items.len == 0);
    for (pos_y_m, 0..) |p, i| {
        // ponytail: branch kept, hits are rare so it predicts well
        if (p < 0) hits.appendAssumeCapacity(.{ .body = @intCast(i) });
    }
}

fn resolveGroundHits(hits: []const Hit, pos_y_m: []f32, vel_y_ms: []f32, kind: []const Kind) void {
    for (hits) |h| {
        assert(h.body < pos_y_m.len);
        pos_y_m[h.body] = 0;
        vel_y_ms[h.body] = -vel_y_ms[h.body] * bounce.get(kind[h.body]);
    }
}

test "rock falls and bounces, balloon rises" {
    const gpa = std.testing.allocator;
    var world: World = .{ .bodies_max = 2 };
    defer world.bodies.deinit(gpa);
    defer world.ground_hits.deinit(gpa);
    try world.bodies.ensureTotalCapacity(gpa, world.bodies_max);
    try world.ground_hits.ensureTotalCapacity(gpa, world.bodies_max);
    world.bodies.appendAssumeCapacity(.{ .pos_y_m = 0.01, .vel_y_ms = -1, .kind = .rock });
    world.bodies.appendAssumeCapacity(.{ .pos_y_m = 1, .vel_y_ms = 0, .kind = .balloon });

    step(&world, 0.1);

    const s = world.bodies.slice();
    try std.testing.expectEqual(@as(f32, 0), s.items(.pos_y_m)[0]);
    try std.testing.expect(s.items(.vel_y_ms)[0] > 0);
    try std.testing.expect(s.items(.pos_y_m)[1] > 1);
}
```

The seam record:

```markdown
### S1: per-kind physics constants

- **Axis**: kind — rock, balloon
- **Evidence**: two cases today: rock, balloon
- **Owner**: sim (src/sim.zig)
- **Rung**: 3 value table. Rung 2 fails because the two kinds need different numbers in the same loop.
- **Cost**:
  - residue: one indexed load per body per transform that reads a kind table
  - change radius for one more case: 0 bodies (one enum tag, one row per table; `EnumArray.init` errors until both rows exist)
  - interface weight: `Kind`
- **Drills passed**: D1, D2, D3, D4 (after restructure)
- **Proof**: n/a (rung 3)
- **Revisit when**: a kind needs behaviour, not numbers (→ a table per kind)
```

The drill table:

| ID | Change | Source | Edits | Radius | Weight Δ | Verdict | Fix |
|---|---|---|---|---|---|---|---|
| D1 | new kind `feather` (falls slowly, no bounce) | user | `Kind`: one tag; `gravity_scale`, `bounce`: one row each | 0 | 0 | pass | — |
| D2 | air drag on everything | user | new `applyDrag(vel_y_ms, dt_s)`; one spine line before `integrateBodies` | 0 | 0 | pass | — |
| D3 | drones that steer toward a target | user | new table `drones { body: u32, target_y_m: f32 }` (a handle into `bodies`; being a drone = a row here); new `steerDrones(drones, vel_y_ms)`; one spine line | 0 | 0 | pass | — |
| D4 | balloons pop on ground hit | user | `resolveGroundHits` would branch on kind | 1 | 0 | fail → pass | restructure: `resolveGroundHits` unchanged; new `popBalloons(ground_hits, kind, &despawn_requests)` after it; despawn becomes a request table the owner of `bodies` drains at frame end. Radius 0. |

D4 is the common case: the failure is structural, and the fix is a new transform and a request table, not a climb up the ladder.

## `ARCHITECTURE.md` template

```markdown
# Architecture

<Two sentences: what the program transforms, from what input to what output.>

## Module DAG
<diagram or list; arrows mean "depends on"; leaves first>
Enforced by: <Cargo crates | build.zig module_specs | tests/deps.rs>

## Spines
- `step` (src/sim.zig): applyGravity → integrateBodies → collectGroundHits → resolveGroundHits
- <one line per pipeline>

## Tables
| Table | Owner (file) | Written by | Read by | Capacity | Arena |
|---|---|---|---|---|---|
| bodies | sim.zig | applyGravity, integrateBodies, resolveGroundHits | render | bodies_max | level |
| ground_hits | sim.zig | collectGroundHits | resolveGroundHits | bodies_max | frame |

## Seams
| Seam | Axis | Rung | Record |
|---|---|---|---|
| per-kind constants | kind | 3 value table | S1 |

## Invariants
- All columns of `bodies` have equal length.
- <every invariant enforced by absence: "render never writes a sim table">

## Adding a feature
- New kind that differs in numbers → a row in the value tables in sim.zig.
- New per-frame behaviour → a transform + a spine line.
- Outside code needs to change a table → a request table consumed by its owner.

## Change drill
<the drill table>
```

## Sources

- Schemify `docs/dev/architecture.md` § Change drill and § Adding a feature; `docs/dev/decisions.md` (numbered decisions with dated amendments)
- Alex Kladov, *ARCHITECTURE.md*: matklad.github.io/2021/02/06/ARCHITECTURE.md.html
