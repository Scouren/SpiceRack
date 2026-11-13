//! The covering properties of R74: the shapes of R, C, L and table elements the ngspice row
//! spells and the shared generator rarely or never draws (it puts a resistor's own keys on a C
//! or L, a dynamic value beside a field the B rewrite drops, an expression in a table point:
//! `admits` refuses each, W1100). Each property builds seeded designs of one shape through the
//! IR text (the builder), legalizes them for ngspice (no refusal: the shapes are spellable by
//! construction), writes, reads back and compares by meaning.
//!
//! Oracle: law L5's own comparison (`read(write(x)) ≅ x`, `eq_meaning`); the shapes are the
//! ones `write_elements/admit.rs` admits, and `ir::parse` checks each design first.
//!
//! Contents: tests only.

use spicerack::dialect::{self, DialectRow, Policy, Sources};
use spicerack::ir;

/// Designs per property.
const SEEDS: u64 = 256;

/// A seeded draw (a 64-bit LCG, Knuth's MMIX constants).
struct Draw(u64);

// ---- private ----

/// The draw of `seed`.
fn seeded(seed: u64) -> Draw {
    Draw(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0x5851_F42D_4C95_7F2D)
}

/// A number in `0..n`.
fn below(g: &mut Draw, n: u64) -> u64 {
    g.0 =
        g.0.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
    (g.0 >> 33) % n
}

/// One time in `n`.
fn one_in(g: &mut Draw, n: u64) -> bool {
    below(g, n) == 0
}

/// A static value: a number or an expression of the deck parameter `k`.
fn value(g: &mut Draw, unit: &str) -> String {
    let n = 1 + below(g, 99);
    match below(g, 3) {
        0 => format!("(k * {n}{unit})"),
        _ => format!("{n}{unit}"),
    }
}

/// A small number for a coefficient.
fn coef(g: &mut Draw) -> String {
    format!("{}m", 1 + below(g, 9))
}

/// A static R, C or L with the keys its device takes: the resistor's own (`tce`, `ac`,
/// `noisy=false`) on an R only.
fn static_passive(g: &mut Draw, k: usize) -> String {
    let (kind, letter, unit) = [
        ("resistor", 'R', "k"),
        ("capacitor", 'C', "p"),
        ("inductor", 'L', "u"),
    ][below(g, 3) as usize];
    let mut f = Vec::new();
    let keys: &[&str] = match letter {
        'L' => &["nt"],
        _ => &["l", "w"],
    };
    if one_in(g, 4) {
        f.push(format!(
            "params=({}=1u)",
            keys[below(g, keys.len() as u64) as usize]
        ));
    }
    if one_in(g, 3) {
        f.push(format!("mult={}", 2 + below(g, 3)));
    }
    f.push(format!("value={}", value(g, unit)));
    if letter != 'R' && one_in(g, 3) {
        f.push("ic=0.5 ic_mode=uic_only".into());
    }
    for key in ["tc1", "tc2"] {
        if one_in(g, 3) {
            f.push(format!("{key}={}", coef(g)));
        }
    }
    if letter == 'R' && one_in(g, 3) {
        f.push(format!("tce={}", coef(g)));
    }
    if one_in(g, 3) {
        f.push(format!("temp_abs={}", 30 + below(g, 50)));
    }
    if one_in(g, 3) {
        f.push(format!("dtemp={}", 1 + below(g, 9)));
    }
    if letter == 'R' && one_in(g, 3) {
        f.push(format!("ac={}", value(g, "k")));
    }
    if one_in(g, 3) {
        f.push(format!("scale={}", 2 + below(g, 3)));
    }
    if letter == 'R' && one_in(g, 3) {
        f.push("noisy=false".into());
    }
    let ic_mode = f.iter().position(|s| s.starts_with("ic="));
    if let Some(at) = ic_mode {
        // `ic_mode` is the last field of the text form
        let ic = f.remove(at);
        let (ic, mode) = ic.split_once(' ').expect("ic and its mode");
        f.insert(at, ic.to_string());
        f.push(mode.to_string());
    }
    format!("  element {kind} {letter}{k} (n{k} 0) {}\n", f.join(" "))
}

/// An R, C or L whose value reads the circuit, with the fields the B rewrite keeps: `m`, `tc1`
/// and `tc2` from 27 °C (reciprocal on a C), the noise switch on an R.
fn dynamic_passive(g: &mut Draw, k: usize) -> String {
    let (kind, letter, unit) = [
        ("resistor", 'R', "k"),
        ("capacitor", 'C', "p"),
        ("inductor", 'L', "u"),
    ][below(g, 3) as usize];
    let n = 1 + below(g, 99);
    let value = match below(g, 3) {
        0 => format!("(v(c) * {n}{unit})"),
        1 => format!("({n}{unit} + v(c, n{k}) * 1{unit})"),
        _ => format!("({n}{unit} * (1 + i(vc)))"),
    };
    let mut f = Vec::new();
    if one_in(g, 3) {
        f.push(format!("mult={}", 2 + below(g, 3)));
    }
    f.push(format!("value={value}"));
    let mut has_tc = false;
    for key in ["tc1", "tc2"] {
        if one_in(g, 2) {
            f.push(format!("{key}={}", coef(g)));
            has_tc = true;
        }
    }
    if letter != 'R' || one_in(g, 2) {
        f.push("noisy=false".into());
    }
    if has_tc {
        f.push("tc_base=fixed27".into());
        if letter == 'C' {
            f.push("tc_law=reciprocal".into());
        }
    }
    format!("  element {kind} {letter}{k} (n{k} 0) {}\n", f.join(" "))
}

/// An E or G table with number points (increasing inputs), a G's multiplier.
fn table(g: &mut Draw, k: usize) -> String {
    let current = one_in(g, 2);
    let mut x = -(below(g, 5) as i64);
    let points: Vec<String> = (0..2 + below(g, 4))
        .flat_map(|_| {
            x += 1 + below(g, 3) as i64;
            [x.to_string(), format!("{}.{}", below(g, 10), below(g, 10))]
        })
        .collect();
    let (letter, tail) = if current {
        ('G', " output=current")
    } else {
        ('E', "")
    };
    let mult = if current && one_in(g, 2) {
        format!(" mult={}", 2 + below(g, 3))
    } else {
        String::new()
    };
    format!(
        "  element table {letter}{k} (n{k} 0){mult} input=(v(c)) points=[{}] law=ngspice_rounded{tail}\n",
        points.join(", ")
    )
}

/// Runs `shape` over the seeds: each design of 1 to 4 elements of the shape (with the node
/// `c`, its source `vc` and the parameter `k` they may read), parsed, legalized without a
/// refusal, written, read back and equal by meaning; returns the elements built.
fn round_trips(shape: fn(&mut Draw, usize) -> String) -> usize {
    let mut built = 0;
    for seed in 0..SEEDS {
        let mut g = seeded(seed);
        let mut ir = String::from(
            "file \"p.sp\" deck\n  title \"p\"\n  param k = 2\n  element vsource vc (c 0) dc=1\n",
        );
        for k in 0..1 + below(&mut g, 4) as usize {
            ir.push_str(&shape(&mut g, k));
            built += 1;
        }
        let mut design = ir::Design::new();
        let mut diags = ir::Diags::new();
        ir::parse(&ir, &mut design.builder(), &mut diags);
        assert!(diags.is_empty(), "seed {seed}: {diags:?}\n{ir}");
        let mut legal = design.clone();
        dialect::legalize(&mut legal, row(), &mut diags);
        let refused: Vec<String> = diags
            .rows()
            .iter()
            .filter(|d| dialect::refuses(d.code))
            .map(|d| format!("{} arg {}", d.code, d.arg))
            .collect();
        assert!(refused.is_empty(), "seed {seed}: refused {refused:?}\n{ir}");
        let text = dialect::write(&legal, legal.entry(), row(), &mut diags);
        let mut back = ir::Design::new();
        let mut read = ir::Diags::new();
        dialect::read(
            &Sources::one("p.sp", &text),
            row(),
            Policy::Lenient,
            &mut back,
            &mut read,
        );
        let mut dumped = String::new();
        ir::dump(&back, back.entry(), &mut dumped);
        assert!(
            back.eq_meaning(&design),
            "seed {seed}:\n{ir}written as\n{text}\nread back as\n{dumped}{read:?}"
        );
    }
    built
}

/// The ngspice row.
fn row() -> &'static DialectRow {
    dialect::by_name("ngspice").expect("the ngspice row")
}

#[test]
fn static_passives_with_their_own_keys_round_trip_by_meaning() {
    // floor: every seed ran, at least one element each
    let built = round_trips(static_passive);
    assert!(built >= SEEDS as usize, "{built} built");
}

#[test]
fn dynamic_passives_with_the_kept_fields_round_trip_by_meaning() {
    let built = round_trips(dynamic_passive);
    assert!(built >= SEEDS as usize, "{built} built");
}

#[test]
fn tables_with_number_points_round_trip_by_meaning() {
    let built = round_trips(table);
    assert!(built >= SEEDS as usize, "{built} built");
}
