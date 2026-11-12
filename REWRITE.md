# REWRITE.md: intentional behaviour changes of the IR rewrite

The old pipeline (`src/syntax/render` and what feeds it, at tag `oracle-ir0`) is the oracle of
the rewrite. The differential harness compares old and new per (input, target): the status
(written or refused, and the refusal's class), the meaning (`read_B(old text)` against
`read_B(new text)`), and on a schedule the numbers both texts give on simulator B.

**Rule: a difference is a line in this file, or it is a bug.** A bug found in the old pipeline
is logged here with its evidence, not fixed in the old code.

Line format, one per difference class (not per deck):

```
<id> | <pairs: src→tgt, or *> | <construct> | old: <what the old pipeline does> | new: <what the new one does> | evidence: <INV-semantics row, ledger row, manual clause, or run> | decks: <count and list file, when known>
```

## Decided before any dialect was written (REWRITE-PLAN §10, `rewrite/RULINGS.md`)

| id | pairs | construct | old | new | evidence |
|---|---|---|---|---|---|
| RW-001 | * | number spelling | an ngspice-read number is respelled as the shortest text ngspice reads back to the same double; a built number in SI form; other targets from the token's spelling | every number is the literal as written (digits, point, exponent or scale), spelled with the same digits on every target | RULINGS R1; INV-semantics 1.08, 1.09 |
| RW-002 | * | statement order | statements placed by kind (loads, globals, functions, includes, parameters, models, subcircuits, sections, body, testbench, `.end`) | item order of the source, per scope; a target that needs loads first hoists exactly those | REWRITE-PLAN §3.1 (order is data); INV-constructs A32 |
| RW-003 | * | nested subcircuits | hoisted to the top as `<parent>__<name>` with references renamed, on every target | kept nested where the target has nested definitions; hoisted by a pass where it does not | INV-constructs B3 |
| RW-004 | * | a subcircuit head the reader cannot read | head kept as text, body dissolved into the parent, `.ends` kept raw | one `Opaque` for the whole block, or a refusal; never a dissolved body | INV-constructs B7 |
| RW-005 | * | lines the source's own simulator rejects | dropped at read with a note | an `Opaque` of class `InvalidInSource` (same-family writers copy it, others refuse) | INV-constructs L12 |
| RW-006 | * | diagnostic codes | W/R/P numbers allotted per work stream | one table per owner in its own range; the facade maps back to the old numbers until the old pipeline is retired | REWRITE-PLAN §4.5 |
| RW-007 | Python | deck building | every builder call formats an ngspice card; the deck is parsed back | builders append IR rows; no text, no re-parse; diagnostics locate by element and Python call site | INV-python §2, §7.1 C2 |
| RW-008 | Python | the store (`save`, `to_source`, `load`) | ngspice text behind a header line | IR text | INV-python §1.12; REWRITE-PLAN §3.6 |
| RW-009 | * | user-declared dialects (`DialectDef`, `spicerack.def_`, hooks over written card text) | 114 definition calls selecting code paths of two family writers | not ported; a dialect implements `Read`/`Write` | REWRITE-PLAN §10.4 |
| RW-010 | * | marker comments read by the backends (`*spicerack ngbehavior=`, the LTspice per-analysis marker) and their scanners over the written deck | facts carried as comment lines and re-scanned | run facts handed over beside the text | REWRITE-PLAN §6.1; INV-python §5.3 |

## Found by the differential (appended as pairs are flipped)

*(none yet)*
