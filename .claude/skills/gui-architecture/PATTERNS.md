# GUI patterns, priced

Every pattern below occupies one position on one of two independent axes. **State flow** says who writes the state and how a change reaches the screen. **Rendering mode** says what survives between frames. A framework ships one position on each axis, but the two are separable: egui is immediate mode, and it can carry a one-way MVU flow just as well as a tangle of `&mut` bindings.

Each pattern is priced on the three cost axes from `codebase-clean-code`:

- **Runtime residue**: work the seam leaves in the hot path next to concrete code.
- **Change radius**: existing function bodies a drilled change edits. New files, functions, rows, variants and spine lines count as 0.
- **Interface weight**: concepts a caller must learn (types, ordering rules, error modes, registration).

A request table is rung 6 of that skill's ladder: it leaves residue once per request, never once per element.

Each also gets a testability verdict and its idle behaviour. Residue entries here describe structure (what runs per change or per frame). They are not timings; the only measured numbers are the Schemify rows in [Canvas](#canvas-straight-from-tables), and they carry their source.

## Axis 1: state and update flow

Fowler names the two ways a screen learns about a change:

- **Flow synchronization**: the code that made the change also updates the screen.
- **Observer synchronization**: the screen subscribes and is told.

His verdict on the second is the reason this skill prefers one-way flow: "you can't tell what is happening by reading the code … Observer behavior is hard to understand and debug because it's implicit behavior."

| Pattern | Who writes the model | How the screen syncs | Residue | Change radius of a new screen action | Interface weight | Testable headless | Idle |
|---|---|---|---|---|---|---|---|
| **MVC** (Smalltalk-80) | controller calls model methods | observer: views subscribe to the model | one notification per subscriber per change, with the fan-out invisible at the call site | new controller method plus every view that must react | model, view, controller, and the subscription protocol | model yes; controller only with a fake view | sleeps; notifications only on change |
| **MVP, Supervising Controller** | presenter | widgets bind simple fields, and the presenter handles the complex parts | as MVC for the bound part | presenter method plus view interface method | view interface, presenter, binding rules | presenter, through a fake view interface | sleeps |
| **MVP, Passive View** | presenter | flow: the presenter pushes every value into dumb widgets | none beyond the pushes | presenter method plus view interface setter | view interface, presenter | almost everything, since the view is humble | sleeps |
| **MVVM** (Presentation Model plus data binding) | view model setters; two-way bindings write back | observer: `PropertyChanged` / bindings | one handler call per bound property per change, with cascades possible | view model property plus binding markup; derived properties must re-raise | view model, binding syntax, change notification rules, converters, commands | view model yes ("unit tests for the view model … without using the view", Microsoft) | sleeps |
| **MVU / Elm** | `update(model, msg)` only | flow: `view(model)` is re-run after each update | the view rebuild per update, plus the diff if the rendering mode diffs | one `Msg` variant, one `update` arm, view code | `Model`, `Msg`, `update`, `view`, plus commands and subscriptions for effects | `update` and `view` are pure, so all of it | sleeps; `subscription` declares every wake-up source |
| **Flux / Redux** | stores or reducers, via dispatched actions | flow from store to view | a dispatch per action, and every reducer sees every action | one action type, one reducer arm | action, dispatcher or store, reducer, selectors | reducers are pure | sleeps |
| **Immediate-mode `&mut` binding** (`ui.checkbox(&mut app.flag)`) | the widget call, inline | none needed, since the next frame reads the field | none | one widget call | the widget function | only by driving frames | depends on the loop (see axis 2) |

What the table implies:

- **Data binding is hidden control flow.** A setter that raises `PropertyChanged` runs other code synchronously, mid-operation. Microsoft's MVVM guide tells you to raise it only "when the object is known to be in a safe state" because "cascading changes" can follow. Each binding is a write path that a grep of the model's call sites cannot see. Replace a two-way binding with a request row the owner applies: the write becomes visible in the spine, and there is again one owner.
- **MVU is the house shape written as a pattern.** `Msg` is a request table's row type, `update` is the owner's apply transform, and `view` is the humble view. Flux and Redux are the same thing, with several stores and a dispatcher fanning out to them. Redux's three principles, "single source of truth", "state is read-only … emit an action", and "changes are made with pure functions", map to single owner, request table, and pure apply.
- **Immediate mode's `&mut` is a two-way binding at the call site.** It is visible, unlike observer binding. It still lets widget code write any field, which breaks single ownership once the state is a shared table. Fit it by handing the widget a *copy* and emitting a request when the copy changes. `rust.md` and `zig.md` show this for egui, dvui, and raygui.
- **Passive View is the testability ceiling of the classic patterns.** Fowler: "By testing the presenter or by testing the presentation model, you test most of the risk of the UI without having to touch the hard-to-test widgets." The humble view goes one step further: the view itself is a pure function, so even it can be tested without widgets.

## Axis 2: rendering mode

Raph Levien describes every UI as "a pipeline of tree transformations": app data → widget tree → render objects → layout → draw tree → pixels. Each stage is either a stored structure or a per-frame trace. The rendering modes differ in which stages are stored.

| Mode | What persists between frames | Identity across frames | Residue per frame | Change radius of a new widget kind | Interface weight | Testable headless | Idle |
|---|---|---|---|---|---|---|---|
| **Immediate** (egui, dvui, raygui, Dear ImGui) | a small id → state map (hot/active, scroll, text cursor), plus last frame's sizes | hashed ids (`@src()`, labels, `id_salt`) | the whole visible UI re-runs layout and draw on every frame it renders | one function | widget functions, id rules | by driving frames through a test harness | the library is idle-capable. The *loop* decides whether it sleeps, and a fixed-rate loop burns CPU forever |
| **Retained widget tree** (Qt, GTK, Masonry, Slint's item tree) | widget objects with their own state | object identity | only invalidated widgets re-layout and repaint | a widget class with its paint, layout, and event methods | widget lifecycle, parent and child ownership, invalidation calls | through the toolkit's test harness | sleeps on invalidation flags |
| **Declarative diff** (iced, Xilem, React; gpui elements over retained entities) | the retained widget tree, plus view state; the view tree is rebuilt | position, keys, or Xilem's id path | rebuild the view tree plus diff it against the last one | a widget plus its `diff`/`rebuild` | view trait, element types, identity rules | `update` and `view` pure; widgets through a simulator | sleeps between messages |
| **UI-as-data tables** (Schemify `ui_nodes`) | the node table (SoA rows: kind, parent, props, owner), written only on requests | row id (an interned public id) | layout plus scene build when a row or the input changed; nothing otherwise | one `Kind` variant plus its layout and render arms (compiler-listed) | the node table schema and request kinds | the whole frame spine, with no window | sleeps; the spine runs only on input or a due wake-up |
| **Scene straight from tables** (the canvas) | the model tables plus caches keyed by row (tiles, index) | table row handles | draw only the rows in view; cached tiles are copied, not redrawn | one drawing arm per primitive kind | the primitive table schema | golden images plus a perf budget | sleeps |

What the table implies:

- **Immediate mode is an API style, not a frame rate.** The Dear ImGui wiki makes the point directly: the paradigm is about who retains state. It does not require continuous redraw. Muratori's original pitch is that "all your data is stored in the application" and the widget is a function call (`do_button(…)` returning whether it was clicked), tracked by hot and active ids. The library stores very little. What costs you is the loop around it, not the paradigm.
- **Immediate mode has a layout lag.** From egui's README: "to know the size of the window, we must do the layout, but the layout code also checks for interaction … This is a fundamental shortcoming of immediate mode GUIs." The fix is to store last frame's size, which gives "a frame-delay for the correct layout, producing occasional flickering". dvui states the same thing: a new widget "will typically receive a zero-sized rectangle, draw nothing on the first frame, and draw normally on the second". A headless egui context asks for two immediate repaints before it settles (measured in `rust.md`), which fits this settling pass; the cause was not traced in egui's source.
- **The diff exists to recover incrementality that the rebuild threw away.** Levien calls VDOM diffing "a form of half-incrementality". Xilem keeps three trees: a short-lived view tree, a retained widget tree, and view state. "The view tree is retained only long enough to assist in event dispatching and then be diffed against the next version." Identity is the hard part: Flutter uses keys, Compose uses positional memoization, and Xilem uses id paths.
- **UI-as-data replaces the diff with ownership.** The node table changes only when a request is applied, so "what changed" is the set of rows the owner wrote this frame, and nothing has to be rediscovered by comparison. A row hash diff is still available for caches (Schemify's `CanvasDiff`). Accessibility is one transform from the table into an AccessKit `TreeUpdate`. The cost of this mode is that you build the widget behaviours yourself (text editing, scrolling, popups). It pays off when every element must be restyled or replaced, for example by plugins (Schemify D4, D6).
- **Accessibility pushes immediate mode toward a hybrid.** Levien: "a UI toolkit cannot be considered production-ready unless it supports accessibility", and for that "pure immediate mode cannot be used, only a hybrid approach". AccessKit is built for this: "suitable for immediate-mode GUI toolkits, as long as they can provide a stable ID for each UI element".

## The grid: where each framework sits

The positions are as of 2026-10; check them against each project, because they move.

| | Immediate | Retained widgets | Declarative diff | UI-as-data |
|---|---|---|---|---|
| **MVU / request tables** | egui, dvui or raygui with copy-and-request (rust.md, zig.md) | Slint callbacks that only push requests | **iced**; Xilem with callbacks that only push requests | **Schemify `ui`** (winit + taffy + parley + accesskit + vello) |
| **Observer / binding** | — | **Slint** (reactive property bindings) | **gpui** (`Entity<T>` + `cx.notify()` + `Render`) | — |
| **Inline `&mut`** | **egui, dvui, raygui** | — | **Xilem** (callbacks get `&mut State`) | — |

Bold marks a framework's native position. The first row holds the shapes the decision procedure asks for. Getting a framework from its native position into that row takes discipline (copy-and-request, or callbacks that only push), not a rewrite. gpui is the hardest to move, because `Entity::update` hands out `&mut` to any holder of the handle.

## Canvas straight from tables

A canvas with 100k objects is a data problem: build its scene from the tables directly, never as 100k widgets. These are the techniques Schemify measured on the way to its D39 budget (pan and zoom ≥ 120 fps at 100k visible primitives, idle CPU 0%). The numbers come from Schemify `docs/dev/perf.md` (2026-10-11, RTX 4060; unverified here: Schemify's own measurements, not re-run for this skill):

| Technique | What it does | Measured at 100k objects / 260k primitives (p50 / p95 / p99, ms CPU) |
|---|---|---|
| Re-encode every frame (the baseline) | the full scene is rebuilt per frame | pan 10.9, edit 112 |
| **World-anchored tiles** | a pan copies the tiles it already has and records only the uncovered ones; a zoom stretches old tiles, then re-records them within a per-frame cost budget | pan 0.092 / 0.116 / 0.403 |
| **Spatial index** (`PrimIndex`, a CSR grid of about 16 rows per cell) | a tile reads only the rows its world box meets | (part of the tile numbers) |
| **Level of detail** | rows of 2 px or less on screen draw as whole pixels (bit masks, one rect per run), not as paths | (part of the tile numbers) |
| **Row-diff patches** (`CanvasDiff`, a Myers diff of per-row hashes) | after an edit, redraw only the world boxes of rows that changed | GPU edit 1.98 / 3.4 / 3.4 |
| **Compose-only frames** | when nothing outside the layers changed, blit the last target and compose layers as textured quads | (avoids the vector renderer entirely) |

The canvas reads the model's tables through a derived primitive table (`canvas_prims`, owned by `app`, rebuilt by `scene::update` only for changed rows). It never holds model state of its own.

## Idle, by construction

A GUI is idle at 0% CPU when nothing schedules a frame. Three things schedule frames:

- input events;
- timers that the app's tables justify (a run in flight, a pending plugin timer, an animation that is still moving);
- wake-ups posted by worker threads.

Each framework exposes the same three:

- **winit**: `ControlFlow::Wait` / `WaitUntil` / `Poll`. `Poll` is the one that burns CPU.
- **egui**: `request_repaint` / `request_repaint_after`, with idle reported as `repaint_delay == Duration::MAX`.
- **iced 0.14**: reactive rendering redraws only "when either an `update` occurs, or a widget explicitly requests it" (PR #2662). The `unconditional-rendering` feature brings back the old always-redraw behaviour.
- **Masonry**: passes run only when invalidation flags are set.
- **dvui**: `Window.end()` returns `null` to mean "wait for event"; the loop calls `waitTime`.
- **raylib**: `EnableEventWaiting()` makes `EndDrawing` block on input. Without it the loop runs at `SetTargetFPS`.

Make the wake-up a pure function of tables, `wake_ms(&tables) -> Option<u32>`, so a test can assert it is `None` when nothing is pending.

## Sources

- Martin Fowler, *GUI Architectures*: martinfowler.com/eaaDev/uiArchs.html
- Evan Czaplicki, *The Elm Architecture*: guide.elm-lang.org/architecture
- Redux, *Three Principles*: redux.js.org/understanding/thinking-in-redux/three-principles
- Flux, *In-Depth Overview*: facebookarchive.github.io/flux/docs/in-depth-overview
- Microsoft, *Model-View-ViewModel* (.NET MAUI patterns): learn.microsoft.com/en-us/dotnet/architecture/maui/mvvm
- Casey Muratori, *Immediate-Mode Graphical User Interfaces* (2005): caseymuratori.com/blog_0001. No transcript exists; the hot/active details come from the Chalmers TDA361 IMGUI slides (2011) and the Dear ImGui wiki *About the IMGUI paradigm*.
- Raph Levien: *Towards a unified theory of reactive UI* (2019), *Towards principled reactive UI* (2020), *Xilem: an architecture for UI in Rust* (2022), *Advice for the next dozen Rust GUIs* (2022), all at raphlinus.github.io
- egui README (github.com/emilk/egui); dvui `readme-implementation.md`; iced PR #2662; Masonry `pass_system.md`; AccessKit README
- Schemify `docs/dev/architecture.md` (frame spine, request tables, D4, D5, D6, D39) and `docs/dev/perf.md`
