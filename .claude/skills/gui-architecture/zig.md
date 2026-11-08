# Zig GUI ecosystem, fitted to the procedure

The status below is as of **2026-10-16 (check current)**. Zig 0.17.0 was released on 2026-10-01, and every library here moved to it within two weeks. Pin a commit, and expect the next Zig release to move them again.

Snippet markers:

- **verified**: built and its tests run with Zig 0.17.0 against the commit named; `zig fmt --check` clean.
- **type-checked**: analysed by `zig build-obj -fno-emit-bin` against the library's Zig sources, but not linked or run.
- **(unverified)**: not built.

## Status and axis positions

| Library | Version (date) | Zig | State flow (native) | Rendering mode | Idle | Layout | AccessKit | Headless test harness |
|---|---|---|---|---|---|---|---|---|
| **dvui** | v0.5.0 (2026-10-13) is the last release for Zig 0.16.0; main is `0.6.0-dev` | main: `minimum_zig_version = "0.17.0"` | inline `*T` ("directly uses your app's bool"); per-widget state that persists goes through `dataGet`/`dataSet` | immediate, with a retained space for dialogs and toasts. Backends: SDL2/3, SDL3GPU, raylib, DX11, GLFW/OpenGL, wio, pugl, web | `Window.end()` returns micros until the next frame, or `null` for "wait for event"; feed it to `waitTime`. `dvui.refresh` wakes the loop, also from other threads | own single pass (min size + expand/gravity); last-frame min size, so a new widget draws nothing on its first frame | yes, with `-Daccesskit` | `dvui.testing` + the `testing` backend (no rendering): `moveTo(tag)`, `click`, `writeText`, `pressKey`, `settle`, `step`, `expectVisible`; hashed snapshots, which the docs call "unstable and both backend and platform dependent" |
| **raylib** | 6.2 (2026-10-15) | `build.zig.zon` at 6.2 needs 0.17.0 | none; it is a drawing and input library | immediate drawing between `BeginDrawing`/`EndDrawing`. 6.0 added the `rlsw` software renderer and `PLATFORM_MEMORY` (headless, frames exported to images) | `EnableEventWaiting()`: "waiting for events on EndDrawing(), no automatic event polling". Without it the loop runs at `SetTargetFPS` | none | not mentioned in the README (unverified absence) | none built in. `PLATFORM_MEMORY` + `rlsw` give software-rendered frames for goldens (unverified: not built here) |
| **raygui** | 5.0 (2026-07-20); master header says `5.1-dev` | with raylib | inline `*T`; 5.0 makes every control return an `int` result | immediate, "no retained data" | the raylib loop's | manual rectangles | none mentioned | none |
| **raylib-zig** | no releases; `devel` branch, commit 3c56875 (2026-10-16), `.version = "6.0.0"` | `minimum_zig_version = "0.17.0"` | as raylib/raygui | as raylib | `rl.enableEventWaiting()` | — | — | — |

## The core pattern, std only (verified)

This is the same frame as `rust.md`'s, rewritten in DOD-style Zig with fixed capacities and no allocator: an owned `Items` table with one `apply`, a request union, a humble `view` writing `UiNodes` with the click action stored as data, input routing, and an idle early return. Verified with `zig test frame.zig` on Zig 0.17.0: 2 tests passed.

```zig
//! One GUI frame over owned tables: events in, requests to the owner, a pure view out.
const std = @import("std");
const assert = std.debug.assert;

/// `items` table, owned here: only `apply` writes it.
pub const Items = struct {
    pub const capacity = 256;
    len: u16 = 0,
    label: [capacity][]const u8 = undefined,
    done: [capacity]bool = undefined,

    /// Applies `requests` in order; returns whether any row changed.
    pub fn apply(items: *Items, requests: []const ItemRequest) bool {
        assert(items.len <= capacity);
        for (requests) |r| switch (r) {
            .add => |s| if (items.len < capacity) {
                items.label[items.len] = s;
                items.done[items.len] = false;
                items.len += 1;
            },
            .toggle => |i| {
                assert(i < items.len);
                items.done[i] = !items.done[i];
            },
        };
        return requests.len > 0;
    }
};

/// A request table row: what the UI asks the owner to do.
pub const ItemRequest = union(enum) { add: []const u8, toggle: u16 };

pub const Kind = enum(u8) { label, checkbox };

/// `ui_nodes` table, rebuilt by `view`; the click action is stored as data.
pub const UiNodes = struct {
    pub const capacity = Items.capacity + 1;
    len: u16 = 0,
    kind: [capacity]Kind = undefined,
    checked: [capacity]bool = undefined,
    on_click: [capacity]?ItemRequest = undefined,
};

/// Presenter logic: pure, tested without a window.
pub fn doneCount(items: *const Items) u16 {
    var n: u16 = 0;
    for (items.done[0..items.len]) |d| n += @intFromBool(d);
    return n;
}

/// The view: tables in, `ui_nodes` out.
pub fn view(items: *const Items, out: *UiNodes) void {
    out.kind[0] = .label; // row 0: the summary label
    out.checked[0] = false;
    out.on_click[0] = null;
    out.len = 1;
    for (0..items.len) |i| {
        out.kind[out.len] = .checkbox;
        out.checked[out.len] = items.done[i];
        out.on_click[out.len] = .{ .toggle = @intCast(i) };
        out.len += 1;
    }
    assert(out.len == items.len + 1);
}

pub const row_h: f32 = 24;

/// Input routing: a click on row y becomes that node's stored request.
pub fn routeInput(ui: *const UiNodes, clicks_y: []const f32, out: []ItemRequest) []ItemRequest {
    var n: usize = 0;
    for (clicks_y) |y| {
        const row: usize = @intFromFloat(@max(y, 0) / row_h);
        if (row >= ui.len or n == out.len) continue;
        if (ui.on_click[row]) |r| {
            out[n] = r;
            n += 1;
        }
    }
    return out[0..n];
}

pub const App = struct {
    items: Items = .{},
    ui: UiNodes = .{},
    request_buf: [64]ItemRequest = undefined,

    /// The frame spine. Returns whether to redraw; no events means no work.
    pub fn frame(app: *App, clicks_y: []const f32) bool {
        if (clicks_y.len == 0) return false;
        const requests = routeInput(&app.ui, clicks_y, &app.request_buf);
        const changed = app.items.apply(requests);
        if (changed) view(&app.items, &app.ui);
        return changed;
    }
};

fn appWith(labels: []const []const u8) App {
    var app: App = .{};
    for (labels) |l| _ = app.items.apply(&.{.{ .add = l }});
    view(&app.items, &app.ui);
    return app;
}

test "click toggles through the owner" {
    var app = appWith(&.{ "milk", "eggs" });
    try std.testing.expect(app.frame(&.{row_h * 2 + 1}));
    try std.testing.expectEqualSlices(bool, &.{ false, true }, app.items.done[0..2]);
    try std.testing.expectEqual(1, doneCount(&app.items));
}

test "idle frame does no work" {
    var app = appWith(&.{"milk"});
    try std.testing.expect(!app.frame(&.{}));
}
```

## dvui

**Axes:** inline `*T` × immediate. **Fit:** use dvui as the humble view.

- Pass widgets a *copy* of the table value, and emit a request when the widget reports a change.
- Have the owner apply the requests after the view, once per frame.
- Draw a canvas from the table with dvui's path and triangle drawing inside one box. Never create a widget per object.
- Use `@src()` plus `.id_extra = row` for widget ids in loops, and use `.tag` so that `dvui.testing.moveTo` can find a widget.

**Idle:** run the loop as `beginWait` → frame → `end()` → `waitTime(end_micros)` → the backend's `waitEventTimeout`. With nothing animating and no `refresh` call, `end()` returns `null`, and the backend sleeps until input. The second test below asserts this on a settled frame.

Verified against dvui main at commit 0b3768f (2026-10-15) with the `testing` backend (`b.dependency("dvui", .{ .backend = .testing })`, module `dvui_testing`), on Zig 0.17.0: `zig build test` gave 2/2 tests passed. A deliberately wrong expectation made the click test fail, which confirms that it runs.

```zig
const std = @import("std");
const dvui = @import("dvui");

const Items = struct {
    label: [2][]const u8 = .{ "milk", "eggs" },
    done: [2]bool = .{ false, false },
};
const ItemRequest = union(enum) { toggle: u8 };

var items: Items = .{};
var requests: [8]ItemRequest = undefined;
var requests_len: usize = 0;

/// dvui as the rendering mode only: reads the table, writes requests.
fn itemsUi(t: *const Items) void {
    for (t.label, t.done, 0..) |label, done, i| {
        var shown = done; // a copy: the widget's write goes nowhere
        if (dvui.checkbox(@src(), &shown, label, .{ .id_extra = i, .tag = label })) {
            requests[requests_len] = .{ .toggle = @intCast(i) };
            requests_len += 1;
        }
    }
}

fn frame() !dvui.App.Result {
    itemsUi(&items);
    for (requests[0..requests_len]) |r| switch (r) {
        .toggle => |i| items.done[i] = !items.done[i],
    };
    requests_len = 0;
    return .ok;
}

test "click becomes a request" {
    var t = try dvui.testing.init(.{});
    defer t.deinit();
    try dvui.testing.settle(frame);
    try dvui.testing.moveTo("eggs");
    try dvui.testing.click(.left);
    try dvui.testing.settle(frame);
    try std.testing.expectEqual([2]bool{ false, true }, items.done);
}

test "idle settles" {
    var t = try dvui.testing.init(.{});
    defer t.deinit();
    try dvui.testing.settle(frame);
    const wait = try dvui.testing.step(frame);
    try std.testing.expectEqual(null, wait); // null = sleep until the next event
}
```

The `build.zig` wiring:

```zig
const std = @import("std");

pub fn build(b: *std.Build) void {
    const target = b.standardTargetOptions(.{});
    const optimize = b.standardOptimizeOption(.{});
    const dvui_dep = b.dependency("dvui", .{ .target = target, .optimize = optimize, .backend = .testing });
    const mod = b.createModule(.{ .root_source_file = b.path("lab.zig"), .target = target, .optimize = optimize });
    mod.addImport("dvui", dvui_dep.module("dvui_testing"));
    const t = b.addTest(.{ .root_module = mod });
    b.step("test", "").dependOn(&b.addRunArtifact(t).step);
}
```

## raylib + raygui

**Axes:** none for state × immediate. raylib leaves state flow entirely to you, so the procedure applies unchanged:

- tables plus requests;
- a `view` that calls raygui controls with copies;
- one apply per frame.

`rl.enableEventWaiting()` is the whole idle story: `endDrawing` blocks until input. Anything with a timer, such as a running job or an animation, must call `rl.disableEventWaiting()` while the timer is pending and turn waiting back on when `wake_ms` returns `None`, because raylib's wait takes no deadline.

Type-checked against raylib-zig `devel` at commit 3c56875, on Zig 0.17.0:

```sh
zig build-obj -lc -fno-emit-bin \
  --dep raylib=raylib-zig --dep raygui -Mroot=main.zig \
  -Mraylib-zig=lib/raylib.zig --dep raylib-zig -Mraygui=lib/raygui.zig
```

It was not linked or run, because that needs raylib's C build plus a display.

```zig
const rl = @import("raylib");
const rg = @import("raygui");

const Items = struct {
    label: [2][:0]const u8 = .{ "milk", "eggs" },
    done: [2]bool = .{ false, false },
};
const ItemRequest = union(enum) { toggle: u8 };

/// raygui as the rendering mode only: reads the table, writes requests.
fn itemsUi(items: *const Items, out: []ItemRequest) []ItemRequest {
    var n: usize = 0;
    for (items.label, items.done, 0..) |label, done, i| {
        var shown = done; // a copy: the widget's write goes nowhere
        const y: f32 = 10 + 30 * @as(f32, @floatFromInt(i));
        _ = rg.checkBox(.{ .x = 10, .y = y, .width = 20, .height = 20 }, label, &shown);
        if (shown != done and n < out.len) {
            out[n] = .{ .toggle = @intCast(i) };
            n += 1;
        }
    }
    return out[0..n];
}

pub fn main() void {
    rl.initWindow(320, 240, "items");
    defer rl.closeWindow();
    rl.enableEventWaiting(); // endDrawing blocks until input: idle CPU ~0
    var items: Items = .{};
    var buf: [8]ItemRequest = undefined;
    while (!rl.windowShouldClose()) {
        rl.beginDrawing();
        rl.clearBackground(.ray_white);
        for (itemsUi(&items, &buf)) |r| switch (r) {
            .toggle => |i| items.done[i] = !items.done[i],
        };
        rl.endDrawing();
    }
}
```

## Sources

- dvui README, `readme-implementation.md`, `src/testing.zig`, `src/Window.zig` (`end`, `waitTime`), release v0.5.0: github.com/david-vanderson/dvui
- raylib 6.2 `src/raylib.h` and the 6.0 release notes: github.com/raysan5/raylib
- raygui README and `src/raygui.h`: github.com/raysan5/raygui
- raylib-zig README, `build.zig.zon`, `lib/raygui.zig`: github.com/raylib-zig/raylib-zig
- Zig release index: ziglang.org/download/index.json
