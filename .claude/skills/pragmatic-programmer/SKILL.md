---
name: pragmatic-programmer
description: Pragmatic Programmer decision heuristics — DRY, orthogonality, reversibility, contracts, tracer bullets.
disable-model-invocation: true
---

> Abstraction choices in systems code (which mechanism, at what cost) belong to `codebase-clean-code` and its `LADDER.md`; where this skill and that one disagree, that one wins.

# Pragmatic Programmer

## When to Use

- Writing a new module, function, or API boundary
- Refactoring or restructuring existing code
- Reviewing a diff for design smells
- Choosing between abstractions, wrappers, or direct calls
- Deciding how much to polish before shipping
- Any design decision where "it depends" is the honest answer

## Core Principles

### 1. DRY — Knowledge Duplication, Not Code Duplication

DRY is about duplicated **knowledge** (business rules, algorithms, intent), not duplicated text. Two identical lines serving different domain purposes are NOT a violation. Merging them creates coupling where none existed.

**Before extracting "duplicates," ask:** do these change for the same reason? If no, leave them separate.

### 2. Orthogonality

A change in module A should not force changes in module B unless B genuinely depends on A's contract. Test: "if I change X, how many unrelated files break?"

**Apply by:** keeping side effects at boundaries, passing data not services, avoiding global mutable state.

> **DOD tension:** Orthogonality favors encapsulation; DOD favors flat data layouts that cross module lines. Resolve by: orthogonal *interfaces*, flat *data*. The struct-of-arrays can live in one place while multiple systems read slices of it through narrow views.

### 3. Tracer Bullets

Build one thin end-to-end path through the system first — real code, real connections, real deployment. Then widen. This is NOT a prototype (prototypes get thrown away). Tracer code ships.

**Apply by:** wiring the full path (input -> processing -> output) with minimal logic, then filling each stage incrementally.

### 4. Reversibility

Treat decisions as temporary. Hard-coded values, locked-in vendors, baked-in formats — each is a wall you might need to move through.

**Apply by:** wrapping external dependencies at boundaries, using config for values that could plausibly change, preferring data-driven behavior over code-driven behavior.

### 5. Pragmatic Paranoia

Don't trust inputs, don't trust callers, don't trust yourself from last Tuesday. Assert invariants. Crash on impossible states rather than limping forward with corrupt data.

**Apply by:** adding asserts for "this should never happen" conditions, validating at trust boundaries, using types to make illegal states unrepresentable.

### 6. Small Steps, Fast Feedback

Take small, deliberate steps. The rate of feedback is your speed limit. If you can't verify a change works within minutes, the step is too big.

### 7. Good Enough Software

Know when to stop. Software that ships and solves the problem beats perfect software that doesn't exist. "Good enough" is a feature, not an apology — but never compromise on data integrity or security.

## Anti-Patterns

### BAD: Merging coincidentally identical code (False DRY)

```python
# Two handlers that happen to format the same way today
def format_invoice(data):
    return f"{data['name']}: ${data['amount']:.2f}"

def format_receipt(data):
    return f"{data['name']}: ${data['amount']:.2f}"

# WRONG: "they're identical, extract!"
def format_line_item(data):  # Now invoices and receipts are coupled
    return f"{data['name']}: ${data['amount']:.2f}"
```

```python
# GOOD: They look the same but change for different reasons. Leave them separate.
# When invoice formatting adds tax and receipt formatting adds refund info,
# you won't have to untangle a shared function.
```

### BAD: Leaking implementation across boundaries (Orthogonality)

```rust
// WRONG: caller knows about the DB schema
fn get_user(db: &Database) -> Vec<(i64, String, String, bool)> {
    db.query("SELECT id, name, email, active FROM users")
}

// GOOD: boundary returns domain type, DB details stay inside
fn get_user(db: &Database) -> Vec<User> {
    db.query("SELECT id, name, email, active FROM users")
      .map(|row| User::from(row))
      .collect()
}
```

### BAD: Swallowing impossible states (Pragmatic Paranoia)

```typescript
// WRONG: silent corruption
function processOrder(order: Order) {
  if (order.items.length === 0) {
    return; // just... nothing? caller thinks it succeeded
  }
}

// GOOD: crash on contract violation
function processOrder(order: Order) {
  if (order.items.length === 0) {
    throw new Error("processOrder: invariant violated — empty order should be unreachable");
  }
}
```

### BAD: Building the cathedral before laying one brick (Tracer Bullets)

```rust
// WRONG: perfect parser, perfect AST, perfect codegen... no working pipeline
// 3 weeks of work, nothing runs end to end

// GOOD: ugly but connected
fn compile(src: &str) -> Vec<u8> {
    let tokens = lex(src);       // handles 3 token types
    let ast = parse(&tokens);    // handles 1 node type
    let code = codegen(&ast);    // emits 2 instructions
    code // ponytail: runs end-to-end on "1 + 2", expand from here
}
```

## Quick Reference

| Situation | Heuristic |
|---|---|
| Two code blocks look identical | Ask: do they change for the same reason? If no, not DRY — leave them. |
| Adding a dependency/library | Can stdlib or an already-installed dep do it? Wrap the call at the boundary. |
| Designing a new module boundary | "If I change this module's internals, what else breaks?" Minimize that set. |
| Choosing how much to build | Tracer bullet: one thin end-to-end path first, then widen. |
| Hard-coding a value | Will this plausibly change? Config/const if yes. Inline literal if genuinely fixed. |
| Something "should never happen" | Assert it. Crash > silent corruption. |
| Unsure whether to ship or polish | Ship if it solves the user's problem. Polish what's painful, not what's ugly. |
| Tempted to add an abstraction | Is there a second use case RIGHT NOW? No? Inline it. |
