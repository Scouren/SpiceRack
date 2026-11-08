# Legacy Code — Dependency-Breaking Examples

## Parameterize Constructor / Function

Inject the dependency instead of creating it internally.

**BAD — hardcoded dependency, untestable:**
```rust
struct OrderProcessor;
impl OrderProcessor {
    fn process(&self, order: &Order) -> Result<()> {
        let db = PostgresDb::connect("prod-url"); // untestable
        db.save(order)?;
        let mailer = SmtpMailer::new(); // untestable
        mailer.send_confirmation(order)?;
        Ok(())
    }
}
```

**GOOD — inject dependencies, test with fakes:**
```rust
struct OrderProcessor<D: DataStore, M: Mailer> {
    db: D,
    mailer: M,
}
impl<D: DataStore, M: Mailer> OrderProcessor<D, M> {
    fn process(&self, order: &Order) -> Result<()> {
        self.db.save(order)?;
        self.mailer.send_confirmation(order)?;
        Ok(())
    }
}
// Test with FakeDb, FakeMailer — no network, no state
```

> **DOD note:** Use generics (`D: DataStore`) over `dyn DataStore` — monomorphized at compile time, no vtable in the hot path. Reserve `dyn Trait` for when you need runtime polymorphism across a collection of heterogeneous implementations.

## Sprout Method / Sprout Class

New behavior goes in a new, fully tested function. Old code calls it. You don't touch the tangled old logic.

**BAD — editing deep inside untested code:**
```python
def process_payment(order):
    # 200 lines of untested spaghetti...
    # you jam your new discount logic in the middle
    if order.has_coupon:  # buried in untested code
        order.total *= 0.9
    # ...more spaghetti
```

**GOOD — sprout the new behavior, test it independently:**
```python
def apply_discount(order: Order) -> Order:
    """Sprouted: new, tested, isolated."""
    if order.has_coupon:
        return order._replace(total=order.total * Decimal("0.9"))
    return order

def process_payment(order):
    # 200 lines of untested spaghetti...
    order = apply_discount(order)  # one-line insertion, minimal risk
    # ...rest unchanged
```

**Why this works:** `apply_discount` is a pure function — data in, data out. Fully testable without mocking anything. The old spaghetti is unchanged except for one call site.

## Skin and Wrap

Wrap the hard-to-test code so you control what goes in and what comes out.

**BAD — testing code that calls the filesystem directly:**
```typescript
function generateReport(): string {
  const data = fs.readFileSync("/etc/config.json"); // side effect
  return transform(JSON.parse(data));
}
```

**GOOD — wrap the side effect, test the logic:**
```typescript
function generateReport(
  readConfig: () => string = () => fs.readFileSync("/etc/config.json", "utf-8")
): string {
  return transform(JSON.parse(readConfig()));
}
// Test: generateReport(() => '{"key": "test-value"}')
```

**Zig variant — comptime dependency injection:**
```zig
fn processData(
    comptime Reader: type,
    reader: Reader,
    buf: []u8,
) !void {
    const n = try reader.read(buf);
    // process buf[0..n]
}
// Test: pass a FixedBufferStream. Prod: pass a File or net.Stream.
// No vtable, no allocation — comptime monomorphization.
```

## Extract Interface

Extract only the methods the caller uses into a trait/interface.

```rust
// Extract ONLY what the caller needs — not the whole class
trait OrderStore {
    fn save(&self, order: &Order) -> Result<()>;
    fn find(&self, id: OrderId) -> Result<Option<Order>>;
}
// PostgresDb implements OrderStore; tests use FakeOrderStore
```

**Rule:** Extract the narrowest interface possible. If the caller only calls `save`, the trait has one method, not twelve.

## Characterization Test Example

```python
def test_tax_calculation_characterization():
    # We don't know if this is "correct" — we know it's what the code does today.
    # If this breaks after our change, we changed existing behavior.
    result = calculate_tax(income=50000, state="CA")
    assert result == 4750.0  # captured from actual run
```

```rust
#[test]
fn characterize_legacy_parser() {
    // Locks in current behavior — even the weird trailing-comma handling
    let result = parse_csv_row("foo,bar,baz,");
    assert_eq!(result, vec!["foo", "bar", "baz", ""]);
}
```

## Decision: Which Technique?

| Situation | Technique | Why |
|-----------|-----------|-----|
| Function creates its own dependencies | Parameterize | Least invasive, pass deps in |
| Giant function, adding new feature | Sprout Method | Don't touch old code, test new code |
| I/O or global state in the way | Skin and Wrap | Isolate the side effect |
| Need to fake a collaborator class | Extract Interface | Narrow trait, swap in test |
| Zig or performance-critical Rust | Comptime/generics | Zero-cost abstraction, no vtable |
