---
name: writing-perfect-verilog
description: Reference for writing, editing, and reviewing synthesizable SystemVerilog RTL that reads top-down. Use when the user writes or edits Verilog/SystemVerilog, reviews RTL, or asks about always blocks, FSMs, latches, reset, width matching, case statements, or clock-domain crossing.
---

RTL is read far more than it is written, and it is read by someone reconstructing hardware in their head from text. **Readability** — a reader rebuilding the whole module in one downward pass, declarations then flops then logic, without jumping around — is the root virtue; every rule below serves it. The rules are not aesthetic. In RTL a readability failure is usually also a *silicon* failure: the same scattered logic that stalls the reader hides the inferred latch, the race, the missed synchronizer. Making the code legible and making it correct are the same act.

**Bold terms** are defined in [`GLOSSARY.md`](GLOSSARY.md); look them up there for the full meaning.

The consensus this skill encodes comes from the lowRISC Comportable style guide (naming, suffixes, formatting, language-feature rules) and Cliff Cummings' Sunburst Design papers (the nonblocking guidelines and the FSM coding style). Prefer **SystemVerilog-2017** (IEEE 1800-2017) for new RTL.

## Downflow

The one principle the rest lean on: a module reads **top-down**, datapath flowing downward. A signal is *declared* above where it is *driven*, and *driven* above where it is *consumed*, so when the reader meets a name its origin is always behind them, never ahead. Implicit net declarations are therefore banned — every signal is declared before use — and declarations sit close to first use so the type is never far from the reader's eye. Break **downflow** and you force the upward jump that this whole skill exists to prevent; it is the disease every failure mode below is a symptom of.

The reader's pass through a well-formed module, each region answering the question the last one raised:

1. **Header** — `module`, parameters, then ports: clock and reset first, then inputs, then outputs, one per line, direction and width visible at a glance.
2. **Params & localparams** — every magic number named here, so the body carries no bare literals.
3. **Typedefs & state enums** — the module's vocabulary, especially the FSM's, before any behaviour.
4. **Declarations** — all signals, each **`_d`/`_q`** pair adjacent, grouped by the block that drives them.
5. **Sequential logic** — the `always_ff` flops. What holds state, made obvious first.
6. **Combinational logic** — the `always_comb` and `assign` that compute the `_d` and the outputs.
7. **Submodule instances** — leaf connections last, once the local fabric is understood.

A signal used above its declaration, an output assigned in three scattered blocks, a param buried mid-body — each is a break in the pass and the first thing to fix.

## dq

The **`_d`/`_q` convention** is the load-bearing **leading word** of readable RTL. `_q` is a signal *as it leaves a flop* — the registered, held value. `_d` is the value *about to enter* that flop on the next clock edge. Read a name, know its timing, without tracing a wire.

The payoff is structural, not cosmetic. Every flop collapses to one trivial, identical `always_ff` that does nothing but copy `_d` into `_q`; *all* the design — the actual logic — lives in `always_comb` blocks computing `_d` from `_q`. Sequential blocks become boilerplate the reader skims; combinational blocks become the one place intent lives. The reader learns where to look and never relearns it.

```systemverilog
// Sequential — boilerplate, skimmed. One block, one job: hold state.
always_ff @(posedge clk_i or negedge rst_ni) begin
  if (!rst_ni) count_q <= '0;
  else         count_q <= count_d;
end

// Combinational — the design. count_d defined on every path (see defaults).
always_comb begin
  count_d = count_q;                 // default: hold
  if (incr_i) count_d = count_q + 1'b1;
end
```

Suffix the rest for the same reason, and reserve each suffix to its one meaning so it stays trustworthy: `_i`/`_o`/`_io` on ports, `_n` on active-low, `_ni` on an active-low input reset, `_q`/`_d` on flopped state, `_e` on an enum type, `_t` on other typedefs. A stray `_q` on a wire that never sees a flop poisons the convention for every future read — **suffix rot** is the failure, and the convention only pays if it is absolute.

## defaults

An unintended **latch** is the canonical RTL bug, and it is a readability failure first: a combinational path where the reader cannot see what a signal becomes. The defence is a rule the reader verifies *locally*, at the top of the block, before reading a single branch:

**Every `always_comb` opens by assigning a default to every signal it drives.** With defaults set first, no `if`/`case` path can leave a signal unassigned, so no latch can form — and the reader confirms latch-freedom by reading one region, not by simulating every branch. This is why `_d = _q` as the first line is idiomatic: "hold unless something below overrides" is both the safe default and the readable one. Every `case` carries a `default:` arm for the identical reason.

```systemverilog
// BAD — inferred latch: no default, the !req_i path leaves grant_d unassigned
always_comb begin
  if (req_i) grant_d = 1'b1;
end

// GOOD — default first; every path defined, latch impossible
always_comb begin
  grant_d = grant_q;                 // default: hold
  if (req_i) grant_d = 1'b1;
end
```

## blocking

The delimiter is not a style choice; it is how the reader knows the timing, and the wrong one makes simulation disagree with synthesis. Cummings' guidelines, distilled to what synthesizable RTL needs:

- **Sequential logic → nonblocking `<=`.** Flops assign with `<=`, which mimics real pipelined-register behaviour and eliminates the races that plague blocking flops.
- **Combinational logic → blocking `=`.** Logic in an `always_comb` assigns with `=`.
- **One delimiter per block.** The block's kind and its delimiter agree, always.
- **One signal, one block.** Each variable is driven from exactly one `always` block, so the reader learns its value in one place — a signal driven from two blocks is **downflow** breaking down, and a multi-driver race in silicon.

Mechanically: `always_ff` for flops, `always_comb` for logic, `assign` for simple continuous wires (prefer `assign` wherever practical) — these three cover all synthesizable logic; bare `always` and `#0` have no place in it. The block type alone tells the reader the timing before they read a line of its body.

Migrating Verilog-2001: `reg`/`wire` → `logic`, `always @(*)` → `always_comb`, `always @(posedge …)` → `always_ff`. This is a correctness upgrade, not spelling: `always @(*)` *legally* infers latches, so a latch-ridden module lints clean (slang `-Weverything`: zero issues); `always_comb` turns the same latch into a tool error.

## fsm

A state machine is where scattered RTL does the most damage, so it earns the strictest shape — the **two-block FSM**, Cummings' preferred style, three regions in downflow order, each a peer the reader meets once:

1. **State register** — the lone `always_ff`, copying `state_d` into `state_q`. Nonblocking only.
2. **Next-state logic** — one `always_comb`, a single `case (state_q)`, every state an arm, computing `state_d`. Opens with `state_d = state_q` (the default) so unlisted transitions hold.
3. **Output logic** — Moore outputs off `state_q` alone; Mealy outputs off `state_q` and inputs. Prefer **registered outputs** where glitch-free or timing-clean outputs matter — an extra flop stage, not combinational outputs that can glitch between states and eat the downstream clock budget. An output derived from a transition is captured *with* the inputs that caused it, in the same edge — recomputed from live inputs a cycle later, it reports whatever the inputs have since become, not what was accepted.

Name states with an `enum` typedef and use only the symbolic names — the enum is the machine's vocabulary and waveform viewers print it, which is half the debugging win (bare parameters or raw binary encodings beside the names defeat it). Keep each FSM in its **own module**, unmingled with other logic, so both the reader and the synthesis/FSM tools can see it whole. One `case`, one default arm, and the machine reads top-to-bottom without a state diagram beside it.

```systemverilog
typedef enum logic [1:0] { StIdle, StRead, StDone } state_e;
state_e state_d, state_q;

always_ff @(posedge clk_i or negedge rst_ni) begin
  if (!rst_ni) state_q <= StIdle;
  else         state_q <= state_d;
end

always_comb begin
  state_d = state_q;                 // default: hold
  unique case (state_q)
    StIdle: if (go_i)   state_d = StRead;
    StRead: if (done_i) state_d = StDone;
    StDone:             state_d = StIdle;
    default:            state_d = StIdle;
  endcase
end
```

## width

Width mismatches are the silent-corruption class of RTL bug — they simulate, they synthesize, they truncate or zero-extend where the reader never looked. Make width explicit everywhere so the reader can check it by eye:

- **Size every literal.** Write `8'd0` / `'0` / `4'hF`, never a bare `0` or `15` in a sized context. `'0` and `'1` fill to context width and are the readable way to say all-zeros/all-ones.
- **Match widths on assignment and on port connections.** Left and right side agree; instance port and connected signal agree. A deliberate width change is written explicitly, not left to implicit resize.
- **No multi-bit signal in a boolean context.** Reduce it (`|vec`, `vec != '0`) so the intent is on the page rather than in the language's coercion rules.
- **Make the base obvious.** Bare numbers are decimal; hex and binary carry their `'h`/`'b`. Parenthesize any expression a reader would otherwise take to an operator-precedence chart.

```systemverilog
logic [15:0] sum;
logic [7:0]  out_d;
// BAD — silent truncation + unsized literal: high byte dropped, 32-bit 1 resized, no warning the reader sees
assign out_d = sum + 1;

// GOOD — widths written; the truncation is a visible decision
assign out_d = sum[7:0] + 8'd1;
```

## case

`case` is where priority, completeness, and don't-cares hide, so it earns its own discipline:

- **`unique case`** when arms are mutually exclusive and you want the tools to check it — but only when they truly are; a false `unique` is a synthesis/simulation mismatch waiting to fire. **`priority case`** when order matters. Plain `case` when neither guarantee applies.
- **Always a `default:` arm.** It carries the else-everything value and, with the top-of-block defaults, keeps the block latch-free.
- **State completeness/priority with `unique`/`priority`,** which speak to simulation and synthesis alike; the `full_case`/`parallel_case` pragmas (Cummings' "evil twins") tell only synthesis, and the tools disagree about unlisted cases.
- **Wildcards:** `case inside` for wildcard matching in SystemVerilog; `casez` only where Verilog-2001 compatibility forces it (`casex` X-matching hides bugs).

```systemverilog
// BAD — no default (unlisted values latch) + casex (an X in state_q matches any arm)
casex (state_q)
  2'b0?: state_d = StRun;
  2'b10: state_d = StIdle;
endcase

// GOOD — case inside for the wildcard, default arm keeps it latch-free
unique case (state_q) inside
  2'b0?:   state_d = StRun;
  2'b10:   state_d = StIdle;
  default: state_d = StIdle;
endcase
```

`unique case` + `default:` trips slang's `-Wcase-redundant-default`. Keep the default — latch-safety wins — and waive the warning, per this guide's own rule: justified exception, comment, lint waiver.

## reset

Reset is one decision, stated once and applied uniformly, so the reader learns it from the first flop and trusts it holds. Pick **synchronous or asynchronous** for the design and apply that one choice to every flop. Reset only what needs it — control and state, rarely wide datapath — and give every flop in a reset block a defined value in the reset branch; a fall-through is a flop that powers up X. And X is sim-fatal, not cosmetic: `X == 2'b00` evaluates to X, so no comparison ever fires and the machine never starts in simulation. The active-low reset (`rst_ni`, `negedge rst_ni`) is the common convention; whatever you pick, the suffix carries the polarity so no reader has to guess.

```systemverilog
// BAD — mode_q falls through the reset branch: powers up X, and the X propagates
always_ff @(posedge clk_i or negedge rst_ni) begin
  if (!rst_ni) state_q <= StIdle;
  else begin state_q <= state_d; mode_q <= mode_d; end
end

// GOOD — every flop in the block gets a defined reset value
always_ff @(posedge clk_i or negedge rst_ni) begin
  if (!rst_ni) begin state_q <= StIdle;  mode_q <= ModeNorm; end
  else         begin state_q <= state_d; mode_q <= mode_d;   end
end
```

## cdc

A signal crossing clock domains without a synchronizer is a metastability bug that simulates perfectly clean and fails in silicon — invisible to the reader who most needs to see it. So make it the most visible line in the file, not the least. Mark every crossing by name (a suffix or clear prefix), contain each in a **named synchronizer submodule** — two-flop for a single bit, a handshake or gray-coded FIFO for a bus — and feed the first synchronizer flop straight from the source flop, combinational logic on either side of the crossing but not between the domains. The crossing is the most dangerous thing in the module; treat invisibility, not the crossing itself, as the bug.

```systemverilog
// BAD — missing 2FF sync: async signal sampled raw; metastability, simulates clean
always_ff @(posedge clk_i) irq_q <= irq_async;

// GOOD — two-flop synchronizer, crossing named and contained
always_ff @(posedge clk_i) begin
  irq_meta_q <= irq_async;    // stage 1: may go metastable
  irq_sync_q <= irq_meta_q;   // stage 2: safe to consume
end
```

## naming

A name is the cheapest documentation and the only kind that never goes stale. lowRISC's construct casing, which lets a reader infer what a name *is* from how it's written:

- `lower_snake_case` — modules, packages, interfaces, instances, signals, ports, variables, functions, tasks, named blocks.
- `UpperCamelCase` — tunable parameters of parameterized modules; derived localparams.
- `ALL_CAPS` — `` `define `` macros and true constant localparams.
- `lower_snake_case_e` / `lower_snake_case_t` — enum types / other typedefs; `UpperCamelCase` enum members.

Spell concepts out — `rx_fifo_full`, not `rff` — because the reader budgets attention for logic, not decryption. Whole words, descriptive over terse, only the most common abbreviations. End names with a word, and keep clear of reserved words — a `_<number>` tail (`foo_1`) collides with synthesis bus-bit naming and corrupts netlist reading. Group signals that operate together with a shared prefix.

## comments

RTL structure already says *what* — the d/q suffixes, the block types, the FSM shape carry the mechanics. A comment restating them is a **no-op**: `// increment counter` over `count_d = count_q + 1'b1` costs a line and says nothing. Spend comments on what structure cannot show — the *why*: a protocol quirk, a timing assumption, a constant's origin, a workaround for a downstream bug, and any deliberate deviation from this guide (justify every exception in a comment, plus a lint waiver where the tool needs one). Header comments may frame major sections (Controller, Datapath) on a single line; skip the fence-of-slashes banners that clutter without informing. If a comment could be deleted without losing intent, delete it.

## Review checklist

Run these against any module, top to bottom. Each is a reader's-eye check with a **checkable** answer, not a vibe:

- **Downflow** — header, params, types, decls, `always_ff`, `always_comb`, instances, in that order. Any signal used above its declaration, any implicit net, is a break. Every declared port is driven or consumed — a declared-but-unused `rst_n` is a reset that never happens.
- **dq** — flops carry `_d`/`_q`; sequential blocks are boilerplate; all logic lives in comb; suffixes used only in their one meaning.
- **defaults** — every `always_comb` assigns defaults to all its outputs before any branch; every `case` has `default:`. No inferred latch.
- **blocking** — `<=` in every `always_ff`, `=` in every `always_comb`, never mixed; each signal driven from exactly one block; no bare `always`, no `#0`.
- **fsm** — enum states, one next-state `case` with a default, outputs separated, registered where it matters, own module.
- **liveness** — every state escapable (each has an outgoing transition), every asserted output has a deassert path. A state with no exit is a deadlock the form checks above never see.
- **width** — every literal sized; assignment and port widths match; no multi-bit boolean; bases explicit.
- **case** — `unique`/`priority` used honestly; `default:` present; no `full_case`/`parallel_case`; no `casex`.
- **reset** — one strategy across the module; every reset flop reset to a defined value.
- **cdc** — every crossing named and inside a synchronizer; no logic between domains.
- **naming & comments** — casing matches construct; names descriptive; comments explain *why*, not *what*; exceptions justified.

## Failure modes

Diagnose a hard-to-read (and usually hard-to-verify) module by fingerprint; full definitions in [`GLOSSARY.md`](GLOSSARY.md). Each traces back to a broken **downflow**.

```systemverilog
// Scatter — one signal, two blocks; reader must merge them (silicon face: multi-driver race)
always_comb if (a_i) x = 1'b1;
always_comb if (b_i) x = 1'b0;

// Inferred latch — missing default; fix at the top of the block, never in the branch
always_comb if (en_i) y = d_i;

// Timing surprise — delimiter contradicts block kind; simulation and synthesis disagree
always_comb sum <= a + b;

// Silent truncation — 16 bits into 8, quietly (see width)
assign out8 = sum16;

// Magic number — 3072 of what? wanted a named localparam up top
if (cnt_q == 12'd3072)

// Silent crossing — clk_a signal sampled raw in clk_b; no synchronizer, no marking
always_ff @(posedge clk_b) x_q <= a_sig;

// Suffix rot — _q on a wire no flop drives; degrades every _q in the file
assign data_q = a_i & b_i;
```
