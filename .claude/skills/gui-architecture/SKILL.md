---
name: gui-architecture
description: GUI architecture as two axes, state flow and rendering mode, settled by a humble view over owned tables. Use when designing or reviewing a GUI app or editor; choosing or fitting a GUI framework (egui, iced, Slint, Xilem, gpui, dvui, raygui); weighing MVC, MVP, MVVM, Elm/MVU, Flux, immediate or retained mode; drawing a canvas of many objects; or making a UI idle at 0% CPU or testable without a window.
---

# GUI architecture: two axes, one humble view

A GUI architecture is two choices that are independent of each other:

- **State flow**: who writes the state and how a change reaches the screen. The options are MVC, MVP, MVVM, MVU/Elm, Flux, and the immediate-mode `&mut`.
- **Rendering mode**: what survives between frames. The options are immediate, retained widget tree, declarative diff, UI-as-data tables, and a scene drawn straight from tables.

A framework comes with one position on each axis. Choose the positions first, then pick a framework that holds them, or one you can bend to them. [PATTERNS.md](PATTERNS.md) prices every position on residue, change radius and interface weight, along with its testability and idle behaviour. Open it when choosing between patterns or when a user names one.

This skill uses the vocabulary of `codebase-clean-code`: **tables**, **transforms**, **the spine**, **owner**, **request table**. It adds two words:

- **Humble view**: the view is a pure function from tables to a `ui_nodes` table, an element tree, or a scene. It holds no state that is not in a table and makes no decision worth a unit test. It is Fowler's Passive View taken all the way, and Feathers' Humble Object applied to the screen. Anything worth testing gets moved out of it.
- **Frame**: one run of the frame spine. It is triggered by input, by a due wake-up, or by a worker's post, and never by a clock.

## The house position

| Axis | Position | Why |
|---|---|---|
| State flow | **one-way**: events → request tables → the owner's apply → derived tables → view | Each table has one writer, every change appears as a spine line, and there is no observer synchronization to trace in a debugger |
| Rendering, chrome | UI-as-data `ui_nodes` table, or an immediate-mode library used as the humble view | The view stays a pure function either way; full restyling and plugins need the table |
| Rendering, canvas | the scene is built straight from tables | Node count is bounded by the screen, not by the model |
| Time | event-driven; `wake_ms(&tables)` decides the next tick | Idle CPU is 0% by construction |

The frame spine. This is Schemify's, cut down; its full version is in `docs/dev/architecture.md`, "Pipelines":

```text
frame (input, a due wake-up, or a worker post; never a clock):
  route_input(ui_layout, keymap, events)        → command_requests
  dispatch(commands, command_requests)          → edit_requests, ui_requests, run_requests
  apply_edits(model, edit_requests)             → model tables, journal      (the owner)
  recompute(model)                              → derived tables             (incremental)
  view(model, derived, ui_state)                → ui_nodes                   (humble)
  layout(ui_nodes, ui_styles)                   → ui_layout
  build_scene(model, derived, ui_layout, camera) → scene                     (canvas from tables)
  submit(scene); access_tree(ui_nodes, ui_layout) → TreeUpdate
  wake_ms(tables)                               → sleep until input or the deadline
```

## Designing a GUI, or adding a screen

### 1. Model tables

List every value the UI shows or edits, and put each one in a named table with one owner. That includes selection, open documents, tool state, which dialog is open, and the text being edited once it is committed. Values that are only visual may stay inside the framework: hover, focus ring, caret blink, scroll inertia, animation phase. Write those down in a list as well.

Done when every value is either a row in an owned table or on the framework-local list, and no widget, view model or framework object is the only home of a value the app saves, undoes, or sends to another module.

### 2. One-way updates

Every interaction becomes a row in a request table: a `Msg` enum in iced, a `Vec<Request>` that egui or dvui code appends to, or a command row. Only the owner's apply transform writes the table. Replace each two-way binding (MVVM `{Binding Mode=TwoWay}`, an immediate-mode `&mut field`, a Slint `<=>` onto app state) with one of these:

- a copy that is handed to the widget, plus a request emitted when the copy changes;
- a callback that only pushes a request.

Each binding is a write path that a grep of the owner cannot find.

Done when a search for writes to model tables from view, widget or callback code finds none, every request variant has exactly one apply arm, and the spine shows each apply in order.

### 3. Humble view

Write the view as `view(&tables…) -> nodes`, or as `view(&tables…, &mut ui_nodes)`. It reads tables and writes only its own output. Calls into an immediate-mode library count as the output.

Done when the view's signature has no `&mut` to a model or derived table, and a test that calls it twice on the same tables gets equal output (equal `ui_nodes`, equal element-tree query results, or equal pixels).

### 4. Presenter logic as pure functions

Pull every decision out of the view into a pure function over tables. That covers display strings, enable and disable rules, validation, sort and filter order, which rows are visible, hit-test math, and keymap lookup. This is what MVP's presenter and MVVM's view model held. Here it is a plain set of functions with no view interface behind it.

Done when each function has a table-driven test that runs without a window, and the view body holds only placement, calls to these functions, and request pushes.

### 5. Canvases straight from tables

Any collection that can pass about a thousand visible items is drawn from its table into the scene. Examples: schematic objects, plot samples, list rows, timeline clips. Lists can instead be virtualized, so that only rows in the viewport become nodes. The tools are a spatial index, level of detail, world-anchored tiles, and row-diff patches. They are in [PATTERNS.md](PATTERNS.md) under "Canvas straight from tables", with Schemify's measured numbers.

Done when the node and widget count is bounded by what fits on screen, and a release-mode test at the target scale (Schemify uses 100k objects) asserts the frame budget for pan, zoom, select and edit.

### 6. Event-driven, zero when idle

Frames run only on input, on a deadline from `wake_ms(&tables) -> Option<u32>`, or on a post from a worker. Map `None` to the framework's sleep:

| Framework | Sleep for `None` |
|---|---|
| winit | `ControlFlow::Wait` |
| egui | no `request_repaint` call |
| iced 0.14 | the default; skip `unconditional-rendering` |
| dvui | `Window.end()` returns `null` |
| raylib | `EnableEventWaiting()` |

Done when a test runs frames with no events and asserts no redraw is scheduled, using the framework's own idle signal (`rust.md` and `zig.md` have verified tests for egui, winit and dvui), `wake_ms` returns `None` once every pending job is finished, and the running app shows 0% CPU in `top` after it settles.

### 7. Tests: frames, goldens, determinism, budget

Four kinds of test, each in CI:

- **Headless frame tests**: drive the real frame spine with synthetic events and assert on tables, not pixels. Framework harnesses: egui_kittest, iced_test, `masonry_testing`, Slint's testing backend, `dvui.testing`.
- **Golden images**: a CPU renderer (vello_cpu, tiny-skia, the dvui or raylib software path) renders fixture states to PNG. Compare against the blessed file with a tolerance (Schemify allows ±2 per channel on at most 0.1% of pixels). Bless only through an environment variable, and on failure write the actual image beside the golden for review.
- **Determinism**: the same event script gives equal tables and byte-identical saves. The view reads no wall clock, no unseeded randomness, and no hash-map iteration order.
- **Performance budget**: release-mode tests assert frame times at the target scale, plus no wake-up when idle. Run them nightly against a baseline that fails on a regression above a stated margin (Schemify uses 10%).

Done when each of the four exists, names the frame or fixture it covers, and fails when its subject breaks. Check that by temporarily breaking one assertion target per kind.

## Choosing or fitting a framework

Pick the framework whose native positions are closest to the house position, then close the gap with step 2's copy-and-request.

- Tool and debug UIs, where you want them small and fast to write: egui or dvui.
- An Elm-shaped application: iced.
- Designer-owned markup or embedded targets: Slint.
- A UI where every element must be restylable or replaceable by plugins: UI-as-data on winit, taffy, parley, accesskit and vello (Schemify D4).

Per-framework positions, idle APIs, test harnesses and verified fitting code:

- [rust.md](rust.md): egui, iced, Slint, Xilem, gpui, and the vello / taffy / parley / accesskit / winit building blocks. Open it whenever a Rust GUI crate is in scope.
- [zig.md](zig.md): dvui and raylib/raygui. Open it whenever a Zig GUI library is in scope.

## Reviewing an existing GUI codebase

Rebuild the frame spine and the ownership map from the code first; a violation only shows up when you hold the code against them. Then check:

- [ ] **owners**: every model table is written by one apply; list each widget, binding or callback that writes it directly
- [ ] **bindings**: every two-way binding and `PropertyChanged`/observer chain is listed with what it triggers
- [ ] **humble view**: the view takes no `&mut` to a model; list each decision inside it that has no test
- [ ] **canvas**: no per-object widget for a collection that can grow past a screen's worth
- [ ] **idle**: no `Poll`, fixed-FPS loop, or unconditional repaint; `wake_ms` (or its equivalent) is a function of tables
- [ ] **tests**: headless frame, golden, determinism and budget tests each exist, or are named as missing

Done when every box has a verdict with a file:line, and each failure names the step above that fixes it.

## Sources

The full source list for the patterns is in [PATTERNS.md](PATTERNS.md); the framework sources are in [rust.md](rust.md) and [zig.md](zig.md).

- Martin Fowler, *GUI Architectures*: martinfowler.com/eaaDev/uiArchs.html
- Evan Czaplicki, *The Elm Architecture*: guide.elm-lang.org/architecture
- Raph Levien, *Xilem: an architecture for UI in Rust*: raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
- Schemify `docs/dev/architecture.md` (frame spine, request tables, UI-as-data: D4, D6; budgets: D39), `docs/dev/testing.md` (`ui`/`render` oracles), `docs/dev/skills/schemify-ui-loop` (the edit → render → critique → bless loop), `crates/app/tests/shell.rs` (`check_golden`)
