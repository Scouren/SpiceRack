# Rust GUI ecosystem, fitted to the procedure

The status below is as of **2026-10-16 (check current)**. Every crate here is pre-1.0 except Slint, and positions move between releases. Versions and dates come from the crates.io API and the projects' own READMEs and changelogs.

Two kinds of snippet appear. A **verified** snippet was compiled and its tests run on Rust 1.95.0 against the version named, with `cargo clippy --all-targets` clean. A snippet marked **(unverified)** was not built, and the marker says why.

## Status and axis positions

| Crate | Version (date) | Self-described maturity | State flow (native) | Rendering mode | Idle | Layout | AccessKit | Headless test harness |
|---|---|---|---|---|---|---|---|---|
| egui + eframe | 0.37.0 (2026-10-15) | pre-1.0 | inline `&mut` | immediate; tessellates to meshes; wgpu or glow | repaints on interaction or request; `repaint_delay == Duration::MAX` when idle | own; last-frame sizes (one-frame lag) | yes | `egui_kittest` (AccessKit queries, `click`, `run`; wgpu snapshots) |
| iced | 0.14.0 (2025-12-07) | "currently experimental" | MVU: `update(&mut State, Message) -> Task`, `view(&State) -> Element` | declarative: fresh `Element` per view, diffed into a persistent `widget::Tree`; wgpu or tiny-skia | 0.14 reactive rendering: redraw only on `update` or a widget's request | own | no (none in Cargo.toml) | `iced_test` (`simulator(view)`, `click`, `into_messages`, `snapshot`) |
| Slint | 1.18.1 (2026-09-21) | stable 1.x API | reactive property bindings (lazy dirty re-evaluation) + Rust callbacks + `Model`/`VecModel` | retained compiled item tree; femtovg, skia, software; experimental vello (1.18) | event-driven; `Window::request_redraw()` | own layouts; `FlexboxLayout` (1.18) backed by taffy 0.10 (unverified: inferred from `i-slint-core`'s dependencies) | yes (winit backend feature) | `i-slint-backend-testing` ("preliminary"; no pixels; `ElementHandle` via accessibility); `Window::take_snapshot()` |
| Xilem + Masonry | 0.4.0 (2025-10-29) | "alpha-quality" | `app_logic(&mut State) -> impl WidgetView<State>`; callbacks get `&mut State` | declarative view tree diffed into Masonry's retained widget tree; vello | Masonry passes do no work unless invalidation flags are set | Masonry's own | yes | `masonry_testing` `TestHarness` (simulated input, time, `assert_render_snapshot`, `MASONRY_TEST_BLESS=1`) |
| gpui (Zed) | 0.2.2 on crates.io (2025-10-22); main has moved well past it | "pre-1.0 … breaking changes" | `Entity<T>` handles owned by `App`; `entity.update(cx, …)`, `cx.notify()` | "hybrid immediate and retained": `Render::render` rebuilds elements each frame over retained entities; Metal / D3D11 / wgpu | `Window::refresh()` marks dirty; full idle semantics not documented (unverified) | taffy | on main | `#[gpui::test]`, `TestAppContext`, `VisualTestContext` (simulated input; no image snapshots found) |

## Building blocks (the UI-as-data stack)

Schemify's custom toolkit (D4) is these five crates plus its own tables. Each one occupies a single stage of Levien's pipeline and has no opinion on state flow, which is why they fit the procedure without bending.

| Crate | Version (date) | Stage | Fitting it |
|---|---|---|---|
| **winit** | 0.30.13 (2026-03-02); 0.31.0-beta.3 | window, input, event loop | `ControlFlow::Wait` (the default) suspends until an event, and `WaitUntil` serves `wake_ms`. `Poll` is the CPU burner; use it only with vsync for continuous animation. `request_redraw` leads to `RedrawRequested`. Input events become the `events` the frame spine reads. |
| **taffy** | 0.15.0 (2026-10-12) | layout (flexbox, grid, block) | Either keep a `TaffyTree` beside `ui_nodes` (Schemify: taffy 0.14 with `taffy_tree`), or implement `TraversePartialTree` + `LayoutPartialTree` + `CacheTree` + `LayoutFlexboxContainer` on your own node table and call `compute_flexbox_layout` / `compute_root_layout`. The second way stores no second tree. The repo examples `custom_tree_vec` and `custom_tree_owned_partial` show the pattern. 0.15 adds `LayoutContainingBlock` and makes `Position::Static` the default. |
| **parley** | 0.12.0 (2026-10-09) | text shaping and line breaking (Fontique, HarfRust, Skrifa, ICU4X); `PlainEditor` for editing | Keep `FontContext` and `LayoutContext` per window, and keep one `Layout` per text row. A changed width only needs `break_all_lines` + `align`; changed text or styles need a new `Layout`. Cache layouts by row hash so that an idle frame shapes nothing. |
| **accesskit** | 0.25.1 (2026-09-25) | accessibility tree, pushed to platform adapters (`accesskit_winit`) | One transform `access_tree(ui_nodes, ui_layout) -> TreeUpdate`, with stable `NodeId`s from row ids. An incremental update lists only new or changed nodes, and each one replaces its predecessor whole. `focus` is sent on every update. See the verified snippet below. |
| **vello** | vello 0.11.0, vello_cpu 0.3.0, vello_gpu 0.3.0 (all 2026-10-02) | 2D vector rendering from a `Scene` encoded per frame | The README calls `vello_cpu` "the most mature choice". `vello_hybrid` was renamed **`vello_gpu`** and "is intended to become the primary renderer for production GPU use". The original compute-shader `vello` "remains an experimental implementation". Schemify puts both behind one compile-time-generic `Painter` trait, using vello_cpu for goldens and as the no-GPU fallback. |

## The core pattern, framework-free (verified)

This is a complete frame over owned tables: a request table, an owner apply, a humble view writing a `ui_nodes` table whose click actions are stored as data, input routing, and an idle early return. The tests are a headless frame test, an idle test, a determinism test and a presenter test. Verified on Rust 1.95.0 with `rustc --edition 2024 --test` (4 passed) and `cargo clippy --all-targets` clean.

```rust
//! One GUI frame over owned tables: events in, requests to the owner, a pure view out.

/// `items` table, owned by this module: only `Items::apply` writes it.
#[derive(Default, PartialEq, Debug)]
pub struct Items {
    pub label: Vec<String>,
    pub done: Vec<bool>,
}

/// A request table row: what the UI asks the owner to do.
#[derive(Clone, PartialEq, Debug)]
pub enum ItemRequest {
    Add(String),
    Toggle(u32),
    ClearDone,
}

impl Items {
    pub const CAPACITY: usize = 1 << 12;

    /// Drains `requests` in order; returns whether any row changed.
    pub fn apply(&mut self, requests: &mut Vec<ItemRequest>) -> bool {
        assert_eq!(self.label.len(), self.done.len());
        let changed = !requests.is_empty();
        for r in requests.drain(..) {
            match r {
                ItemRequest::Add(s) if self.label.len() < Self::CAPACITY => {
                    self.label.push(s);
                    self.done.push(false);
                }
                ItemRequest::Add(_) => {} // full: a declared drop, not a panic
                ItemRequest::Toggle(i) => self.done[i as usize] ^= true,
                ItemRequest::ClearDone => {
                    let mut keep = self.done.iter().map(|d| !d);
                    self.label.retain(|_| keep.next().unwrap());
                    self.done.retain(|d| !d);
                }
            }
        }
        assert_eq!(self.label.len(), self.done.len());
        changed
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Label,
    Checkbox,
    Button,
}

/// `ui_nodes` table, rebuilt by `view`: one row per node, the click action stored as data.
#[derive(Default, PartialEq, Debug)]
pub struct UiNodes {
    pub kind: Vec<Kind>,
    pub text: Vec<String>,
    pub checked: Vec<bool>,
    pub on_click: Vec<Option<ItemRequest>>,
}

/// Presenter logic: a pure function, tested without a window.
pub fn summary(items: &Items) -> String {
    let n = items.done.iter().filter(|d| **d).count();
    format!("{n} of {} done", items.done.len())
}

/// The view: tables in, `ui_nodes` out. No side effects, no callbacks.
pub fn view(items: &Items, out: &mut UiNodes) {
    *out = UiNodes::default();
    let mut push = |kind, text: String, checked, on_click| {
        out.kind.push(kind);
        out.text.push(text);
        out.checked.push(checked);
        out.on_click.push(on_click);
    };
    push(Kind::Label, summary(items), false, None);
    for (i, (label, &done)) in items.label.iter().zip(&items.done).enumerate() {
        push(Kind::Checkbox, label.clone(), done, Some(ItemRequest::Toggle(i as u32)));
    }
    push(Kind::Button, "Clear done".into(), false, Some(ItemRequest::ClearDone));
}

pub const ROW_H: f32 = 24.0;

/// Input routing: a click on row y becomes that node's stored request.
pub fn route_input(ui: &UiNodes, clicks_y: &[f32], out: &mut Vec<ItemRequest>) {
    for &y in clicks_y {
        let row = (y / ROW_H) as usize;
        if let Some(Some(r)) = ui.on_click.get(row) {
            out.push(r.clone());
        }
    }
}

#[derive(Default)]
pub struct App {
    pub items: Items,
    pub ui: UiNodes,
    pub item_requests: Vec<ItemRequest>,
}

/// The frame spine, run only when there is input. Returns whether to redraw.
pub fn frame(app: &mut App, clicks_y: &[f32]) -> bool {
    if clicks_y.is_empty() {
        return false; // idle: no work, no redraw
    }
    route_input(&app.ui, clicks_y, &mut app.item_requests);
    let changed = app.items.apply(&mut app.item_requests);
    if changed {
        view(&app.items, &mut app.ui);
    }
    changed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_with(labels: &[&str]) -> App {
        let mut app = App::default();
        app.item_requests.extend(labels.iter().map(|s| ItemRequest::Add(s.to_string())));
        app.items.apply(&mut app.item_requests);
        view(&app.items, &mut app.ui);
        app
    }

    #[test]
    fn click_toggles_through_the_owner() {
        let mut app = app_with(&["milk", "eggs"]);
        assert!(frame(&mut app, &[ROW_H * 2.0 + 1.0])); // row 2 = "eggs"
        assert_eq!(app.items.done, [false, true]);
        assert_eq!(app.ui.text[0], "1 of 2 done");
    }

    #[test]
    fn idle_frame_does_no_work() {
        let mut app = app_with(&["milk"]);
        assert!(!frame(&mut app, &[]));
    }

    #[test]
    fn same_script_same_tables() {
        let script: &[&[f32]] = &[&[30.0], &[50.0], &[80.0]];
        let run = || {
            let mut app = app_with(&["a", "b", "c"]);
            for clicks in script {
                frame(&mut app, clicks);
            }
            (app.items, app.ui)
        };
        assert_eq!(run(), run());
    }

    #[test]
    fn summary_counts_done() {
        let items = Items { label: vec!["a".into(), "b".into()], done: vec![true, false] };
        assert_eq!(summary(&items), "1 of 2 done");
    }
}
```

## AccessKit from the node table (verified)

accesskit 0.25.1, Rust 1.95.0, 1 test passed, clippy clean. Schemify's production version is `crates/ui/src/access.rs`; it uses interned public ids as `NodeId`s and exports only visible rows.

```rust
use accesskit::{Node, NodeId, Role, Toggled, TreeId, TreeInfo, TreeUpdate};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    Label,
    Checkbox,
}

pub struct UiNodes {
    pub kind: Vec<Kind>,
    pub text: Vec<String>,
    pub checked: Vec<bool>,
}

/// One transform: `ui_nodes` → a full AccessKit tree. Row index + 1 is the node id, so ids are
/// stable as long as rows are (Schemify uses interned public ids instead).
pub fn access_tree(ui: &UiNodes, focus_row: Option<usize>) -> TreeUpdate {
    const ROOT: NodeId = NodeId(0);
    let mut root = Node::new(Role::Window);
    let mut nodes = Vec::with_capacity(ui.kind.len() + 1);
    for (i, (&kind, text)) in ui.kind.iter().zip(&ui.text).enumerate() {
        let id = NodeId(i as u64 + 1);
        let mut n = Node::new(match kind {
            Kind::Label => Role::Label,
            Kind::Checkbox => Role::CheckBox,
        });
        n.set_label(text.as_str());
        if kind == Kind::Checkbox {
            n.set_toggled(if ui.checked[i] { Toggled::True } else { Toggled::False });
        }
        root.push_child(id);
        nodes.push((id, n));
    }
    nodes.push((ROOT, root));
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(ROOT)),
        tree_id: TreeId::ROOT,
        focus: focus_row.map_or(ROOT, |r| NodeId(r as u64 + 1)),
    }
}

#[test]
fn one_node_per_row_plus_root() {
    let ui = UiNodes {
        kind: vec![Kind::Label, Kind::Checkbox],
        text: vec!["1 of 1 done".into(), "milk".into()],
        checked: vec![false, true],
    };
    let t = access_tree(&ui, None);
    assert_eq!(t.nodes.len(), 3);
    assert_eq!(t.nodes[1].1.toggled(), Some(Toggled::True));
}
```

## egui

**Axes:** inline `&mut` × immediate. **Fit:** use egui only as the humble view.

- Give every widget a copy of the table value, and push a request when `.changed()`.
- Apply the requests once per frame in the owner. In eframe that means the `App::ui` body, after the view.
- Keep presenter logic in plain functions that the `ui` code calls.
- For a canvas, allocate one `ui.allocate_painter` region and paint the visible rows from the table with the `Painter`. Never create a widget per object.

**Idle:** egui's README says it "only repaints when there is interaction … or an animation, so if your app is idle, no CPU is wasted". It also tells you to "expect `egui` to take up 1-2 ms per frame" (unverified: the README's own figure, not measured here). The test below measures what a headless `Context` reports: the first two frames ask for `repaint_delay = 0ns`, which is the layout-settling pass, and from the third frame on it reports `Duration::MAX`. The integration (eframe or your winit loop) turns `Duration::MAX` into `ControlFlow::Wait`. A `request_repaint_after` deadline should come from `wake_ms(&tables)`.

Verified on egui 0.37.0 (feature `default_fonts`) and egui_kittest 0.37.0, Rust 1.95.0: 2 tests passed, clippy clean.

```rust
#[derive(Default)]
pub struct Items {
    pub label: Vec<String>,
    pub done: Vec<bool>,
}

#[derive(Clone, PartialEq, Debug)]
pub enum ItemRequest {
    Toggle(u32),
}

impl Items {
    pub fn apply(&mut self, requests: &mut Vec<ItemRequest>) {
        for r in requests.drain(..) {
            match r {
                ItemRequest::Toggle(i) => self.done[i as usize] ^= true,
            }
        }
    }
}

/// egui as the rendering mode only: reads the table, writes requests, mutates nothing.
pub fn items_ui(ui: &mut egui::Ui, items: &Items, out: &mut Vec<ItemRequest>) {
    for (i, (label, &done)) in items.label.iter().zip(&items.done).enumerate() {
        let mut shown = done; // a copy: the widget's write goes nowhere
        if ui.checkbox(&mut shown, label.as_str()).changed() {
            out.push(ItemRequest::Toggle(i as u32));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::{Harness, kittest::Queryable};

    #[derive(Default)]
    struct State {
        items: Items,
        requests: Vec<ItemRequest>,
    }

    #[test]
    fn click_becomes_a_request() {
        let mut s = State::default();
        s.items.label.push("milk".into());
        s.items.done.push(false);
        let mut h = Harness::new_ui_state(
            |ui, s: &mut State| {
                items_ui(ui, &s.items, &mut s.requests);
                s.items.apply(&mut s.requests); // the owner applies, once per frame
            },
            s,
        );
        h.get_by_label("milk").click();
        h.run();
        assert_eq!(h.state().items.done, [true]);
    }

    #[test]
    fn idle_context_asks_for_no_repaint() {
        let ctx = egui::Context::default();
        let items = Items { label: vec!["milk".into()], done: vec![false] };
        let mut out = Vec::new();
        let mut last = None;
        for _ in 0..3 {
            let mut o = ctx.run_ui(egui::RawInput::default(), |ui| items_ui(ui, &items, &mut out));
            o.textures_delta.clear(); // no painter in a headless test
            last = Some(o.viewport_output[&egui::ViewportId::ROOT].repaint_delay);
        }
        assert_eq!(last, Some(std::time::Duration::MAX));
        assert!(out.is_empty());
    }
}
```

## iced

**Axes:** MVU × declarative diff. This is the house state flow out of the box. `Message` is the request row type, `update` is the owner's apply, and `view` is the humble view.

**Fit:**

- Keep `State` as tables rather than a widget-shaped struct.
- Keep `update` a thin `match` that calls owner transforms, and return `Task`s only for effects.
- Declare every timer and worker as a `Subscription`, so that `subscription(&State)` is your `wake_ms`.
- For a big canvas, use one `canvas::Program` (feature `canvas`), or a custom widget whose `draw` reads the table directly through a `Cache`, rather than one `Element` per object.

**Idle:** reactive rendering since 0.14 (PR #2662). Before that, iced could draw "hundreds of identical frames" while the mouse moved, and the `unconditional-rendering` feature restores the old behaviour.

Verified on iced 0.14.0 (`default-features = false`, features `tiny-skia`, `x11`, `thread-pool`; with no windowing feature, softbuffer fails to build) and iced_test 0.14.0, Rust 1.95.0: 1 test passed, clippy clean.

```rust
use iced::Element;
use iced::widget::{checkbox, column, text};

#[derive(Default)]
pub struct Items {
    pub label: Vec<String>,
    pub done: Vec<bool>,
}

/// iced's `Message` is the request table's row type.
#[derive(Clone, Debug)]
pub enum ItemRequest {
    Toggle(u32),
}

/// `update`: the owner applies one request.
pub fn update(items: &mut Items, r: ItemRequest) {
    match r {
        ItemRequest::Toggle(i) => items.done[i as usize] ^= true,
    }
}

/// `view`: a pure function from the table to an element tree.
pub fn view(items: &Items) -> Element<'_, ItemRequest> {
    let n = items.done.iter().filter(|d| **d).count();
    let rows = items.label.iter().zip(&items.done).enumerate().map(|(i, (l, &d))| {
        checkbox(d).label(l.as_str()).on_toggle(move |_| ItemRequest::Toggle(i as u32)).into()
    });
    column![text(format!("{n} done"))].extend(rows).into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use iced_test::simulator;

    #[test]
    fn click_then_update() {
        let mut items = Items { label: vec!["milk".into()], done: vec![false] };
        let mut ui = simulator(view(&items));
        let _ = ui.click("milk");
        for r in ui.into_messages() {
            update(&mut items, r);
        }
        assert_eq!(items.done, [true]);
        assert!(simulator(view(&items)).find("1 done").is_ok());
    }
}
```

## Slint

**Axes:** observer binding × retained. Properties re-evaluate lazily when a dependency changes ("a dependency is registered for any property accessed while evaluating a binding").

**Fit:**

- Treat the `.slint` file as the humble view, and keep app state in Rust tables.
- Expose that state to Slint through `VecModel`/`ModelRc` rows and `set_<prop>` calls written by one Rust transform after each apply. This is flow synchronization from your side, with Slint's bindings running only inside the view.
- Wire every `on_<callback>` to push a request rather than mutate. Drain the requests in one apply, from the same closure, or from `invoke_from_event_loop` for workers.
- Keep two-way `<=>` bindings inside the `.slint` view. Never bind them onto app state.
- Test with `i-slint-backend-testing`. Its version must match Slint exactly (`=x.y.z`), and it renders no pixels. For pixel tests use `Window::take_snapshot()` or the software renderer.

(unverified: no Slint snippet is given; the slint build compiles its own `.slint` compiler, which was not built for this skill.)

## Xilem + Masonry

**Axes:** inline `&mut` callbacks × declarative diff into a retained widget tree. `app_logic(&mut State)` re-runs after events, and the `View` trait's `build` / `rebuild` / `teardown` / `message` methods diff the new view tree against the old one. Events are routed by id path, not by message enum.

**Fit:**

- Have each callback push a request into a `requests` table inside `State`.
- Apply the requests at the top of `app_logic`.
- Then build the view from what is now read-only state.

This keeps one writer per table even though every callback receives `&mut State`.

The project calls itself alpha (0.4.0). Masonry's main branch is moving to an `imaging` abstraction with Vello CPU support and a new layout system (Linebender report, 2026-04).

(unverified: no snippet; building Xilem pulls vello and wgpu, which was not built for this skill.)

## gpui

**Axes:** observer (`cx.notify()` → observers re-render) × hybrid. Elements are rebuilt per frame by `Render::render`, over retained `Entity<T>` state owned by `App`.

**Fit:** this is the hardest to fit, because any holder of an `Entity` handle can `update` it.

- Make one entity per owner.
- Expose a request method that only enqueues, and drain the queue in that entity's own `update`.
- Keep `render` humble: read entities, emit elements, and send listeners that only enqueue.

AccessKit support is on main. Whether the crates.io 0.2.2 release has it is unconfirmed, so a git dependency on Zed's repo may be needed (check current).

(unverified: no snippet; gpui's crates.io release lags main and its platform crates were not built for this skill.)

## winit wake-up (verified)

This is the one function between the frame spine and the event loop. Schemify's version is `crates/render/src/window.rs`, around line 616. Verified on winit 0.30.13 (`default-features = false`, feature `x11`), Rust 1.95.0: 1 test passed, clippy clean.

```rust
use std::time::Duration;
use winit::event_loop::ControlFlow;

/// The app's one wake-up question, asked after every frame: `None` = sleep until input.
pub fn control_flow(wake_ms: Option<u32>) -> ControlFlow {
    match wake_ms {
        Some(ms) => ControlFlow::wait_duration(Duration::from_millis(u64::from(ms))),
        None => ControlFlow::Wait,
    }
}

#[test]
fn idle_waits() {
    assert_eq!(control_flow(None), ControlFlow::Wait);
    assert!(matches!(control_flow(Some(25)), ControlFlow::WaitUntil(_)));
}
```

## Sources

- crates.io API, `https://crates.io/api/v1/crates/<name>`, read 2026-10-16
- egui README and `crates/egui_kittest/README.md`: github.com/emilk/egui
- iced 0.14.0 README, `core/src/widget/tree.rs`, `test/src/lib.rs`, PR #2662: github.com/iced-rs/iced
- Slint README, CHANGELOG v1.18.1, reactivity guide (docs.slint.dev), `internal/backends/testing/README.md`: github.com/slint-ui/slint
- Xilem v0.4.0 release notes, `masonry_core/src/doc/pass_system.md`, `masonry_testing/README.md`: github.com/linebender/xilem; Linebender *This Month in Linebender* #25 (linebender.org/blog/tmil-25)
- gpui README, `_ownership_and_data_flow.rs`, `element.rs`, `window.rs`, `app/test_context.rs`: github.com/zed-industries/zed/tree/main/crates/gpui
- vello README and the sparse-strips-v0.3.0 release: github.com/linebender/vello
- taffy CHANGELOG, `src/tree/traits.rs`, `src/compute/mod.rs`: github.com/DioxusLabs/taffy
- parley `parley/src/lib.rs`: github.com/linebender/parley
- AccessKit README and `TreeUpdate` docs (0.25.1 source): github.com/AccessKit/accesskit
- winit 0.30.13 `src/event_loop.rs`: github.com/rust-windowing/winit
- Schemify `Cargo.toml`, `crates/ui/src/tables.rs` (`UiNodes`), `crates/ui/src/access.rs`, `crates/render/src/window.rs`
