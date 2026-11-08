---
name: refactoring-patterns
description: Fowler refactorings and enterprise patterns — smell catalogue, refactoring moves, repository/service layers.
disable-model-invocation: true
---

> Abstraction choices in systems code (which mechanism, at what cost) belong to `codebase-clean-code` and its `LADDER.md`; where this skill and that one disagree, that one wins.

# Refactoring Patterns

## When to Use

- Refactoring existing code before adding a new feature
- After getting code working (red-green-REFACTOR)
- During code review when you spot structural smells
- Designing data access layers (Repository, Data Mapper, Unit of Work)
- User asks to "clean up", "simplify", or "restructure" code
- You see functions > 20 lines, classes that change for multiple reasons, or repeated field groups

## Core Principles

### Small Safe Steps — Never Big-Bang

Every refactoring is one atomic transformation that preserves behavior. Chain small steps. If a refactoring feels large, decompose it into Extract → Move → Inline sequence. Run tests between each step. Never rewrite a module from scratch when a sequence of extractions achieves the same result.

### Extract Only When the Name Communicates Intent

Do NOT extract a function just because a block is "too long." Extract when the extracted piece has a name that tells the reader WHY, not WHAT. If you cannot name it better than `process_part_2`, leave it inline.

### Refactoring Timing

1. **Before adding a feature:** restructure so the new feature slots in cleanly
2. **After making it work:** clean up the mess you made getting to green
3. **During review:** spot smells, suggest specific refactorings by name
4. **Never:** refactor and change behavior in the same commit

### PEAA Patterns — Pick the Simplest That Works

| Need | Pattern | Use when |
|------|---------|----------|
| Decouple domain from DB | **Repository** | Domain logic must be testable without DB |
| Track changes across a transaction | **Unit of Work** | Multiple entities change atomically |
| Map rows to objects with no ORM coupling | **Data Mapper** | Complex domain model, separate from schema |
| Simple CRUD, schema ≈ domain | **Active Record** | Admin panels, config tables, low complexity |
| Avoid duplicate loads | **Identity Map** | Multiple queries might return same entity |
| Orchestrate use cases | **Service Layer** | Thin layer dispatching to domain objects |

**DOD tension:** Repository and Data Mapper push toward object-per-row thinking. In hot paths, prefer batch operations over per-entity patterns — load arrays of components, not graphs of objects. Use PEAA patterns at system boundaries (API handlers, transaction boundaries), not in inner loops. See Anti-Patterns below.

## Smell → Refactoring Cheat Sheet

| Smell | What's wrong | Fix |
|-------|-------------|-----|
| **Long Method** | Can't grasp in one read | Extract Function (only if name adds clarity) |
| **Feature Envy** | Method uses another class's data more than its own | Move Function to the data it envies |
| **Data Clumps** | Same 3+ fields travel together | Extract into a struct/dataclass |
| **Primitive Obsession** | Raw strings/ints for domain concepts | Replace Primitive with Value Object |
| **Shotgun Surgery** | One change touches 10 files | Move fields/functions to consolidate |
| **Divergent Change** | One class changes for unrelated reasons | Split into focused modules |
| **Middle Man** | Class delegates everything | Inline the middle man |
| **Speculative Generality** | Abstractions for future needs | Delete them. YAGNI. |

## Workflows

### Refactoring an Existing Function

1. Ensure tests exist that cover current behavior (write them if missing)
2. Identify the smell by name from the cheat sheet
3. Apply the smallest refactoring that addresses it
4. Run tests — they must still pass
5. Commit. One refactoring per commit.
6. Repeat if more smells remain

### Introducing a Repository Layer

1. Identify the entity and its current data access (inline SQL, ORM calls scattered in handlers)
2. Create a Repository with methods named after domain operations (`find_active_users`), not SQL (`select_where_active_eq_true`)
3. Move all data access for that entity into the Repository
4. Have callers use the Repository, remove direct DB access
5. **DOD note:** For batch-heavy reads, add batch methods (`find_all_by_ids(ids: &[Id])`) returning `Vec<T>`, not iterator-of-single-loads

### Replace Conditional with Polymorphism

1. Only do this when the conditional switches on TYPE and appears in 3+ places
2. If it appears in 1-2 places, a match/switch is clearer — leave it
3. Create trait/interface, one impl per case, dispatch via the type system

## Anti-Patterns

### BAD: Extracting without a meaningful name

```python
def process_order(order):
    # 40 lines of validation
    _validate_part_1(order)  # what does "part 1" mean?
    _validate_part_2(order)
    # 20 lines of pricing
    _do_pricing(order)
```

### GOOD: Extract communicates WHY

```python
def process_order(order):
    validate_shipping_address(order)
    validate_payment_method(order)
    apply_volume_discounts(order)
```

### BAD: Repository wrapping single-entity loads in a hot path

```rust
// Called 10,000 times in a loop — one query per entity
for id in entity_ids {
    let entity = repo.find_by_id(id).await?;
    results.push(transform(entity));
}
```

### GOOD: Batch operation, DOD-aligned

```rust
// ponytail: single query, process as contiguous slice
let entities = repo.find_by_ids(&entity_ids).await?;
let results: Vec<_> = entities.iter().map(transform).collect();
```

### BAD: Premature polymorphism (one implementation)

```typescript
interface IUserService { getUser(id: string): Promise<User>; }
class UserServiceImpl implements IUserService { /* only impl ever */ }
// factory that returns the only impl
function createUserService(): IUserService { return new UserServiceImpl(); }
```

### GOOD: Direct until you need the seam

```typescript
// ponytail: extract interface when second impl appears, not before
class UserService {
  async getUser(id: string): Promise<User> { /* ... */ }
}
```

### BAD: Refactoring + behavior change in one commit

```
commit: "Extract validation and also add email verification"
```

### GOOD: Separate commits

```
commit 1: "Extract validate_contact_info from process_order"
commit 2: "Add email verification to validate_contact_info"
```

## Quick Reference — Decision Flow

```
Is the code hard to understand?
├─ No → don't touch it
└─ Yes → Can you name the extracted piece meaningfully?
   ├─ No → add a comment, leave it inline
   └─ Yes → Extract Function/Method
      └─ Does the extracted piece use another module's data more?
         ├─ Yes → Move it to that module (Feature Envy)
         └─ No → done

Adding a new feature is hard because of current structure?
├─ Refactor FIRST (separate commit), then add feature
└─ Never interleave refactoring and behavior changes

Choosing an enterprise pattern?
├─ Simple CRUD → Active Record
├─ Complex domain, need testability → Repository + Data Mapper
├─ Multiple entity changes in one transaction → Unit of Work
└─ Hot path, many entities → skip per-entity patterns, use batch queries
```
