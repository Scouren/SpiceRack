---
name: doc-comments
description: Documentation comments that carry a fact the signature cannot. Use whenever writing a `pub` function, type, error set, or module in Rust or Zig; when auditing existing doc comments in a final pass; or when the user mentions rustdoc, doctests, `///`, `//!`, or missing_docs.
---

# Doc comments

A doc comment earns its place by carrying **a fact the signature cannot**. That one test decides whether to write a comment, what goes in it, and whether an existing one survives an audit.

## The test

Read the comment beside the signature and name the fact it adds. `/// Returns the user id.` on `fn user_id() -> UserId` adds nothing; the types already said it. That is **restatement**: it costs maintenance forever and informs no one.

When you cannot name a fact, there are two honest outcomes: rename the item so the signature carries the meaning, or write the one-line summary and stop.

## The fact ladder

Every `///` states at least one of these, ranked by what readers get wrong most:

1. **Precondition**: what the caller must guarantee. Every assert reachable from the public entry, every `unsafe` obligation.
2. **Invalidation**: what this call breaks. Pointers, slices, iterators, handles, previously returned views.
3. **Ownership**: who frees the result, with which allocator, and how long a borrowed input must outlive the result.
4. **Failure**: each error and the condition that produces it, what state is left behind on failure, and what panics.
5. **Cost**: complexity or allocation when it is not O(1) or not what the name implies.
6. **Reason**: the decision that was non-obvious or contested. "Always motivate, always say why" (TIGER_STYLE).

Ranks 1–4 are contract: the caller's code is wrong without them. Rank 6 survives longest, because a reason does not go stale the way a description does.

Ranks 1 and 4 are where this skill meets the `assertions` skill: every assert it adds to a public function is a rank-1 line here (`Asserts that ...` in Zig, `# Panics` in Rust), and every operating error it routes to a return is a rank-4 line.

## The module header

Every file, module, and crate opens with a `//!` header that states two things the declarations below it cannot:

- **In → out**: the transformation the file performs, as data in and data out. `MIR → the same MIR with each pure diamond replaced by straight-line code` (VerA `ifconv.zig`). `Bytes in, bytes and requests out` (Schemify `crates/doc`).
- **What the code cites**: the spec clauses, LRM sections, decision records (`D22`), or papers the code implements, so a reader can check the code against its source. A file that cites nothing says so ("No LRM clause: this checks VerA's own IR").

Then, when they apply: who may import it, what it deliberately does not do and why, and the invariant every row it produces satisfies. VerA's `AGENTS.md` makes this a rule ("Every file opens with a `//!` header: its transformation (in → out) and the LRM clauses its code cites"), and ESPice and Schemify follow the same shape.

## Contract, not implementation

Document what callers may rely on. A `///` that describes *how* the body works is invalidated by every refactor; that is where stale docs come from.

Implementation notes go in `//` comments inside the body, beside the code they explain, read by whoever changes it. The `//` side owns everything addressed to the next maintainer rather than the caller: why this algorithm, what was tried and rejected, the known ceiling, and every tagged marker (`TODO:`, `SAFETY:`, `ponytail:`). A rationale in `///` is published API and binds a reimplementation that was never meant to be bound by it.

## When to write none

- A private helper its name and signature already specify.
- Trivial accessors, `Deref`, derived impls.
- Anything hidden from docs: mark it hidden (`#[doc(hidden)]`, or keep it non-`pub`) rather than writing a comment that explains it is internal.

Every public item is documented, and every comment names its fact. The two pull against each other only when the item's signature is weak, and then the fix is the signature.

## Writing

1. Write or update the file's `//!` header: in → out, then what the code cites.
2. Write each public item's summary line: one sentence, third-person present indicative, ending in a period ("Returns", not "Return"). The summary is what listings show, so it stands alone.
3. Walk the ladder top-down and add every rank that applies. Skipping rank 1 or 2 when it applies is a bug, not a style choice.
4. Document the error set: what each error means and what state the call leaves behind. The language file has the form.
5. Add the example, as a test the toolchain compiles and runs. An example the toolchain never compiles is a comment that will rot.
6. Turn on the enforcement once, at the crate or module root, and run it.

Done when the file has a header naming in → out and its citations, every public item has a summary and every applicable ladder rank, every error has its condition, every example compiles and passes, and the language's doc build prints no warnings.

## Auditing

The pass to run after a feature is written, so the docs describe what shipped rather than what was intended:

1. List the public items the change added or touched, and the files they live in.
2. For each file, check the header still states its in → out and every clause the code now cites. Grep the file for clause and decision references (`§`, `D[0-9]+`) and compare with the header's list.
3. For each item, name the fact its comment carries. Unnamed means restatement: delete it or rename the item.
4. Grep the bodies for `assert`, `panic`, `unreachable`, allocation, and reallocation. Each one reachable from a public entry needs its rank-1, 2, or 3 line; this grep catches the contracts added after the comment was written.
5. Run the language's doc lints and doctests and fix what they report before reading anything by eye.
6. Read the surviving prose for register. Comments that read as generated filler get skipped by the people they were written for; the `humanizer` skill handles this pass, and it runs last, on prose that survived steps 2–5.

Done when every touched file's header matches its citations, every touched public item names its fact, the doc build prints no warnings, and the doctests pass.

## Language files

| Language | File | What that language enforces least |
|---|---|---|
| Zig | [zig.md](zig.md) | ownership and invalidation: with no borrow checker, the comment is the contract. Also the `//!` placement rules and the error-set form |
| Rust | [rust.md](rust.md) | `# Safety`: the compiler demands `unsafe`, never the obligation behind it. Also the section order, the lint table, and the zero-warning gate |

Open the file for the language you are writing before step 4 of Writing or step 5 of Auditing.

Sources: TigerBeetle `docs/TIGER_STYLE.md`; VerA `AGENTS.md` §4 and its `lib/` headers; ESPice `src/` headers; Schemify `docs/dev/contributing.md` and `crates/*/src/lib.rs`; Rust API Guidelines, Documentation chapter (rust-lang.github.io/api-guidelines/documentation.html).
