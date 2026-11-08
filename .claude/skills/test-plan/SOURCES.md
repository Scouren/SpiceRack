# Sources

Ranked within each group. Read the first item of each group first.

## How to write tests
1. **"How SQLite Is Tested"** — sqlite.org/testing.html. The benchmark: 100% MC/DC, OOM/IO fault injection, fuzzing, asserts, branch-instruction mutation, ~200-item release checklist (§10).
2. **Richard Hipp, "Reliability Lessons From SQLite"** — Software Should Work 2026, youtube.com/watch?v=V_qzqY1bb7I, slides sqlite.org/u/ssw6. Origin of the 5-rung ladder and the history of each rung being overtaken. "Testability must be a design objective."
3. **Andrew Kelley, "State of the (Tagged) Union Address"** — Systems Distributed '26, youtube.com/watch?v=zwi5b5xSsKA (≈29:30–34:50). The ladder applied to Zig; caveat at ≈34:00: non-trivial change sends you back to the top.
4. **matklad, "How to Test"** — matklad.github.io/2021/05/31/how-to-test.html. Test features not code, data-driven tests, one `check` function. Also "Generate All The Things" (matklad.github.io/2021/11/07/generate-all-the-things.html) and "Swarm Testing Data Structures" (tigerbeetle.com/blog/2025-04-23-swarm-testing-data-structures, Zig comptime).
5. **Brian Marick, "How to Misuse Code Coverage"** — exampler.com/testing-com/writings/coverage.pdf. Coverage finds gaps, not correctness; faults of omission.
6. **Vladimir Khorikov, *Unit Testing Principles, Practices, and Patterns*** (Manning, 2020). What makes a test valuable; mocks vs real dependencies.
7. ***Software Engineering at Google*, ch. 11–14** — abseil.io/resources/swe-book (free). Test sizes, flakiness, test doubles, large tests.
8. **Kent Beck, *Test-Driven Development: By Example*** (2002). Red-green-refactor origin.
9. **Michael Feathers, *Working Effectively with Legacy Code*** (2004). Seams; getting untested code under test.
10. **Gerard Meszaros, *xUnit Test Patterns*** (2007). Reference catalog of test smells and patterns.
11. **Ham Vocke, "The Practical Test Pyramid"** — martinfowler.com/articles/practical-test-pyramid.html.
12. **Google Testing Blog, "Testing on the Toilet"** — testing.googleblog.com.

## Fuzzing and generated testing
1. ***The Fuzzing Book*** — fuzzingbook.org (free, interactive). Grammar fuzzing, coverage, delta debugging.
2. **google/fuzzing** — github.com/google/fuzzing/tree/master/docs. Writing good fuzz targets, structure-aware fuzzing.
3. **OSS-Fuzz docs** — google.github.io/oss-fuzz. Corpus, dictionaries, regression practice.
4. **Rust Fuzz Book** — rust-fuzz.github.io/book.
5. **John Hughes, "Testing the Hard Stuff and Staying Sane"** (talk) and "Experiences with QuickCheck" (paper). Property and state-machine testing.
6. **Hypothesis docs + David MacIver's essays** — hypothesis.works. Shrinking, what makes PBT work.
7. **Groce et al., "Swarm Testing"** (ISSTA 2012).
8. **Zig release notes**, "Fuzzer" sections — ziglang.org/download/ (0.14.0, 0.16.0, 0.17.0). Smith API, multiprocess, infinite mode, roadmap.
9. **Wasmtime fuzzing** — docs.wasmtime.dev/contributing-fuzzing.html + `fuzz/README.md`. Production oracle-rich differential fuzzing for a compiler.

## Compilers
1. **John Regehr's blog, Embedded in Academia** — blog.regehr.org. Compiler fuzzing, UB, test-case reduction (C-Reduce).
2. **Yang et al., "Finding and Understanding Bugs in C Compilers"** (PLDI 2011, Csmith).
3. **Le et al., "Compiler Validation via Equivalence Modulo Inputs"** (PLDI 2014, EMI).
4. **Lopes et al., "Alive2: Bounded Translation Validation for LLVM"** (PLDI 2021).
5. **Chen et al., "A Survey of Compiler Testing"** (ACM CSUR 2020).
6. Read the suites: Zig `test/`, rustc `tests/ui` + compiletest, LLVM `llvm/test` (lit/FileCheck), wasmtime `fuzz/`.

## Stateful systems
1. **Will Wilson, "Testing Distributed Systems w/ Deterministic Simulation"** (Strange Loop 2014, FoundationDB).
2. **TigerStyle** — github.com/tigerbeetle/tigerbeetle/blob/main/docs/TIGER_STYLE.md. Assertion density, static allocation, simulation-first design. Read the VOPR source (Zig).
3. **Antithesis blog** — antithesis.com/blog. DST in practice.
4. **Jepsen analyses** — jepsen.io.

## Numerical / scientific
1. **Chen et al., "Metamorphic Testing: A Review of Challenges and Opportunities"** (ACM CSUR 2018).
2. **Roache / Salari & Knupp, Method of Manufactured Solutions** (Sandia report SAND2000-1444). Code verification for PDE/DAE solvers.
3. **Goldberg, "What Every Computer Scientist Should Know About Floating-Point Arithmetic"** (1991). Basis for tolerance choice.
4. **Xyce Regression Suite** — github.com/Xyce/Xyce_Regression (2000+ tagged tests). How a production SPICE is tested.
5. **LAPACK Working Note 41** — netlib.org/lapack/lawnspdf/lawn41.pdf §7. Scaled residual-ratio oracles that scale with conditioning.
6. **Bruce Dawson, "There are only four billion floats, so test them all"** — randomascii.wordpress.com (2014). Exhaustive f32.
7. **Rigger & Su, "Finding Bugs in Database Systems via Query Partitioning"** (OOPSLA 2020). The metamorphic oracle that beat SQLite's MC/DC + fuzzing; template for designing metamorphic relations.
8. **Wetzler, Heule, Hunt, DRAT-trim** (SAT 2014). Certificate checking for solvers.

## Gating and release
1. **Graydon Hoare, "The Not Rocket Science Rule of Software Engineering"** — graydon2.dreamwidth.org/1597.html.
2. **SQLite release checklist** — described in sqlite.org/testing.html §10.
3. ***Software Engineering at Google*, ch. 23 (CI)**.
