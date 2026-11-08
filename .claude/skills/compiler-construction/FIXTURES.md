# Conformance fixtures and goldens

Open this when you are writing fixtures for a spec clause, building the fixture harness, or setting up the golden diff for a refactor. The rules come from VerA's harness (`AGENTS.md` §2, §4, §6), with the language-specific directives left out.

## The fixture header

A fixture is a source file in the language being compiled. Its header is machine-read comment lines, written in that language's comment syntax. A minimal key set, generalized from VerA's (`lrm`, `reject`, `xfail`, `checks` and `neighbour` are VerA's own; `expect` stands in for its per-domain value directives):

```
//! lrm 9.4.1                  <- clause this fixture is evidence for; one line per clause
//! <quoted spec sentence>     <- the normative text, verbatim
//! expect V(out) = 0.75       <- derived BY HAND from the sentence, with the derivation
//! checks 6                   <- exactly this many ok= verdicts; derive N from the asserts
//! reject E0310               <- must fail to compile with this code (a substring match:
//!                               always name the code; a bare `reject` matches anything)
//! neighbour legal_9_4_1.va   <- the legal variant that must compile and pass
//! warn W0853                 <- compiles and runs; this warning must appear
//! xfail <reason>             <- the fixture is right and the compiler is not; XPASS fails
```

A verdict is the whole token: `ok=1` passes, and `ok=10` is a failure. A fixture with no `checks` passes only on a transcript that is non-empty and passes everywhere. `checks` can't be combined with `reject`.

## Evidence rules

- **Two-way per clause.** A clause counts as verified only with a positive fixture that runs the compiler's output and checks a value, plus a refusal fixture for its invalid form. Having only the refusal is one-way evidence. "It compiles" is the weakest evidence there is and closes nothing.
- **A refusal has a legal neighbour.** Isolate the one invalid construct, pin its code, and ship the minimal legal variant next to it. The pair is what shows a real restriction rather than a blanket refusal. When a rule has no invalid form, the header says so instead of inventing one.
- **Coverage is measured, never typed.** One script counts clause cites, and only cites from fixtures that actually run. A compile-only fixture moves no number. Documents quote that script's output.
- **Gate on names.** Keep a checked-in verdicts file with one fixture per line. CI fails on a new FAIL, and also on a new PASS until the file is updated. A count can hold steady while its membership changes.
- **Gate on `$?`.** `cmd | tail` swallows the exit status, so use `exit "${PIPESTATUS[0]}"`. When a capture shows 0 names, the capture broke. It doesn't mean the suite passed.

## The trap: a fixture a conforming compiler fails

Making a wrong fixture pass is cheapest by breaking the compiler. VerA found and fixed four such fixtures. One example: a test of `absdelta(V, 0.5)` against a wave that steps by exactly 0.5, while the clause fires on "**more than** delta". So before you change the compiler to pass a fixture, re-derive the fixture's expectation from the spec text. If you withdraw a claim from a fixture, name the fixture it moved to.

## Byte-identical goldens

```sh
zig build golden -- before     # on the pre-change tree: every artifact of every fixture
# ... the refactor phase ...
zig build golden -- after
zig build golden -- diff       # prints IDENTICAL, or exits 1 naming each differing file
```

- Artifacts cover everything a consumer observes: emitted code, rendered diagnostics, the JSON diagnostics, and any `--dump-*` output.
- One phase is one branch and one PR. Bug fixes and behaviour changes land as their own commits, outside the phase.
- If the goldens can't stay identical, stop the phase and re-scope it. The golden diff never gets blessed as part of a refactor.
- Determinism: compile the corpus twice in fresh processes and byte-compare. Ids in insertion order (interning, node ids) and sorted diagnostics are what make this pass.
- Tests move with the code they test, in the same commit.

For a golden's provenance (where each expected value came from) and reviewing snapshot diffs, see `test-plan`'s TECHNIQUES.md, "Golden / snapshot".
