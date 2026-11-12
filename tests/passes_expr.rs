//! The meaning of the expression passes (`src/passes/expr/`): each keeps every value.
//!
//! Oracle: the reference evaluator (`ir::eval_typed`), never the pass's own arithmetic. For each
//! root a pass rewrites, the old tree and the new one (both stay in the design: rows are only
//! appended) are evaluated under the same bindings and must agree bit for bit and tag for tag,
//! or both be a domain error of the same kind. Points: every root of generated designs
//! (`ir::gen::generate`) holding the pass's feature, under bindings drawn from the boundary set
//! (±0, k+½, k+½±1 ulp, 2⁵²±1, 2⁵³, the domain edges, Int and Real tags) and random finite
//! values; plus each pair of the boundary set as the operands of the rewritten node. Bindings are
//! finite: a non-finite operand lies outside every function's domain (R12).
//!
//! Contents: tests only.

#[path = "passes_expr/bindings.rs"]
mod bindings;
#[path = "passes_expr/compare.rs"]
mod compare;
#[path = "passes_expr/towers.rs"]
mod towers;

use spicerack::ir;

use compare::{
    ANY, code, keeps_boundary_values, keeps_folded, keeps_generated_values, keeps_values_over,
    notes, typed,
};

/// Each rule of the powers, roundings and ternaries: its rows, the texts whose roots it
/// rewrites (operands `x` and `y`), and the node it removes.
type Rule = (&'static str, &'static [&'static str], fn(ir::Expr) -> bool);

const RULES: &[Rule] = &[
    (
        "pow_to_powabs",
        &[
            "pow(x, 2)",
            "pow(x, 3)",
            "pow(x, -1)",
            "pow(x, -2)",
            "pow(x, 0.5)",
            "pow(x, 1.5)",
            "pow(abs(x), y)",
            "pow(exp(x), y)",
            "pow(sqr(x), y)",
        ],
        |e| matches!(e, ir::Expr::Call(ir::Func::Pow, _)),
    ),
    (
        "round_half_even_to_away",
        &["round_half_even(x)", "round_half_even(x * y)"],
        |e| matches!(e, ir::Expr::Call(ir::Func::RoundHalfEven, _)),
    ),
    (
        "round_half_away_to_floor",
        &["round_half_away(x)", "round_half_away(x - y)"],
        |e| matches!(e, ir::Expr::Call(ir::Func::RoundHalfAway, _)),
    ),
    ("sgn_to_ternary", &["sgn(x)", "sgn(x - y)"], |e| {
        matches!(e, ir::Expr::Call(ir::Func::Sgn, _))
    }),
    ("step_to_ternary", &["step(x)", "step(x * y)"], |e| {
        matches!(e, ir::Expr::Call(ir::Func::Step, _))
    }),
    (
        "step_half_to_ternary",
        &["step_half(x)", "step_half(x + y)"],
        |e| matches!(e, ir::Expr::Call(ir::Func::StepHalf, _)),
    ),
    ("ramp_to_ternary", &["ramp(x)", "ramp(x - y)"], |e| {
        matches!(e, ir::Expr::Call(ir::Func::Ramp, _))
    }),
    (
        "pow_signed_to_ternary",
        &[
            "pow_signed(x, y)",
            "pow_signed(x, 3)",
            "pow_signed(x - y, 0.5)",
        ],
        |e| matches!(e, ir::Expr::Call(ir::Func::PowSigned, _)),
    ),
    (
        "sqrt_abs_to_sqrt",
        &["sqrt_abs(x)", "sqrt_abs(x - y)"],
        |e| matches!(e, ir::Expr::Call(ir::Func::SqrtAbs, _)),
    ),
    ("ln_abs_to_ln", &["ln_abs(x)", "ln_abs(x * y)"], |e| {
        matches!(e, ir::Expr::Call(ir::Func::LnAbs, _))
    }),
    (
        "explin14_to_ternary",
        &["exp_lin14(x)", "exp_lin14(x * 14 + y)", "exp_lin14(x + 14)"],
        |e| matches!(e, ir::Expr::Call(ir::Func::ExpLin14, _)),
    ),
    ("sqr_to_mul", &["sqr(x)", "sqr(x - y) /: 2i"], |e| {
        matches!(e, ir::Expr::Call(ir::Func::Sqr, _))
    }),
    (
        "pwl_to_ternary",
        &[
            "pwl(x, -1, 2, 0.5, -3, 2.5, 7)",
            "pwl(x * y, 0, 1)",
            "pwl(x, 0, 0, 1, 1)",
        ],
        |e| matches!(e, ir::Expr::Call(ir::Func::Pwl, _)),
    ),
];

// ---- private ----

fn is_powabs(e: ir::Expr) -> bool {
    matches!(e, ir::Expr::Call(ir::Func::PowAbs, _))
}

#[test]
fn powabs_to_pow_keeps_every_value() {
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = pow_abs(x, y)\n";
    let points = keeps_boundary_values("powabs_to_pow", text, ANY);
    let generated = keeps_generated_values("powabs_to_pow", is_powabs, false);
    assert!(generated > 0, "no generated root held pow_abs");
    eprintln!("powabs_to_pow: {points} boundary points, {generated} generated roots");
}

#[test]
fn powabs_to_pow_dyn_keeps_every_value() {
    // `$temp` makes the root dynamic (class Temp, R14); `0 * $temp` adds nothing to y
    let text =
        "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = pow_abs(x, y + 0 * $temp)\n";
    let points = keeps_boundary_values("powabs_to_pow_dyn", text, ANY);
    let generated = keeps_generated_values("powabs_to_pow_dyn", is_powabs, false);
    assert!(generated > 0, "no generated root held a dynamic pow_abs");
    eprintln!("powabs_to_pow_dyn: {points} boundary points, {generated} generated roots");
}

#[test]
fn div_to_div_typed_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Binary(ir::BinOp::Div, ..));
    for (x, y, tags) in [
        ("1i", "2i", &[3u8][..]),
        ("1", "2i", &[2]),
        ("1", "1", &[0]),
    ] {
        let text = format!(
            "file \"p.sp\" deck\n  param x = {x}\n  param y = {y}\n  param r = x / y\n  element behav b1 (n 0) expr=(x / y + v(n))\n"
        );
        for name in ["div_to_div_typed", "div_to_div_typed_dyn"] {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}, {y}): {points} boundary points");
        }
    }
    assert!(typed("div_to_div_typed", hit) > 0);
    assert!(typed("div_to_div_typed_dyn", hit) > 0);
}

#[test]
fn div_typed_to_div_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Binary(ir::BinOp::DivTyped, ..));
    for (x, y, tags) in [
        ("1i", "2i", &[3u8][..]),
        ("1", "2i", &[2]),
        ("1i", "1", &[1]),
    ] {
        let text = format!(
            "file \"p.sp\" deck\n  param x = {x}\n  param y = {y}\n  param r = x /: y\n  element behav b1 (n 0) expr=(x /: y + v(n))\n"
        );
        for name in ["div_typed_to_div", "div_typed_to_div_dyn"] {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}, {y}): {points} boundary points");
        }
    }
    assert!(typed("div_typed_to_div", hit) > 0);
    assert!(typed("div_typed_to_div_dyn", hit) > 0);
}

#[test]
fn a_division_of_mixed_operand_types_is_noted_and_so_is_every_int_quotient() {
    let got = notes("div_typed_to_div", "mixed");
    assert!(
        !got.is_empty() && got.iter().all(|&c| c == code(101)),
        "{got:?}"
    );
}

#[test]
fn toint_to_trunc_keeps_every_value_and_waits_for_the_typed_divisions() {
    let hit = |e| matches!(e, ir::Expr::Call(ir::Func::ToInt, _));
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = to_int(x) + to_int(y)\n  element behav b1 (n 0) expr=(to_int(x) + v(n))\n";
    for name in ["toint_to_trunc", "toint_to_trunc_dyn"] {
        eprintln!(
            "{name}: {} boundary points",
            keeps_boundary_values(name, text, &[0])
        );
    }
    assert!(typed("toint_to_trunc", hit) > 0);
    assert!(typed("toint_to_trunc_dyn", hit) > 0);
    assert_eq!(notes("toint_to_trunc", "typed_division_left"), [code(103)]);
}

#[test]
fn toreal_to_mul_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Call(ir::Func::ToReal, _));
    for (x, tags) in [("1i", &[1u8, 3][..]), ("1", &[0, 2])] {
        let text = format!(
            "file \"p.sp\" deck\n  param x = {x}\n  param y = 1\n  param r = to_real(x) /: 2i\n  element behav b1 (n 0) expr=(to_real(x) /: 2i + v(n))\n"
        );
        for name in ["toreal_to_mul", "toreal_to_mul_dyn"] {
            let points = keeps_boundary_values(name, &text, tags);
            eprintln!("{name} ({x}): {points} boundary points");
        }
    }
    assert!(typed("toreal_to_mul", hit) > 0);
    assert!(typed("toreal_to_mul_dyn", hit) > 0);
}

#[test]
fn sgnzeropos_to_ternary_keeps_every_value() {
    let hit = |e| matches!(e, ir::Expr::Call(ir::Func::SgnZeroPos, _));
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = sgn_zero_pos(x) /: 2i\n  element behav b1 (n 0) expr=(sgn_zero_pos(x) /: 2i + v(n))\n";
    for name in ["sgnzeropos_to_ternary", "sgnzeropos_to_ternary_dyn"] {
        eprintln!(
            "{name}: {} boundary points",
            keeps_boundary_values(name, text, &[0, 2])
        );
    }
    assert!(typed("sgnzeropos_to_ternary", hit) > 0);
    assert!(typed("sgnzeropos_to_ternary_dyn", hit) > 0);
    assert_eq!(notes("sgnzeropos_to_ternary", "typed"), [code(103)]);
}

#[test]
fn the_powers_roundings_and_ternaries_keep_every_value() {
    for &(name, texts, hit) in RULES {
        let dyn_name = format!("{name}_dyn");
        for &(row, dynamic) in &[(name, false), (dyn_name.as_str(), true)] {
            let mut points = 0;
            for &body in texts {
                let field = match dynamic {
                    true => format!("  element behav b1 (n 0) expr=({body} + v(n))\n"),
                    false => format!("  param r = {body}\n"),
                };
                let text = format!("file \"p.sp\" deck\n  param x = 1\n  param y = 1\n{field}");
                // sqr_to_mul decides by the tag table: x's binding keeps its definition's tag
                let tags: &[u8] = if name == "sqr_to_mul" { &[0, 2] } else { ANY };
                points += keeps_boundary_values(row, &text, tags);
            }
            let generated = keeps_generated_values(row, hit, false);
            eprintln!("{row}: {points} boundary points, {generated} generated roots");
            // generated pwl points are rarely increasing literals: its own test draws them
            assert!(
                generated > 0 || name == "pwl_to_ternary",
                "{row}: no generated root held its node"
            );
        }
    }
}

#[test]
fn sqr_to_mul_keeps_an_int_square_real() {
    // oracle: eval_typed of sqr (Real, no 2⁵³ bound) at Int operands up to 2⁵³
    let text = "file \"p.sp\" deck\n  param x = 1i\n  param y = 1\n  param r = sqr(x)\n";
    for name in ["sqr_to_mul", "sqr_to_mul_dyn"] {
        let body = text.replace(
            "sqr(x)",
            if name == "sqr_to_mul" {
                "sqr(x)"
            } else {
                "sqr(x) + $temp * 0"
            },
        );
        let points = keeps_boundary_values(name, &body, &[1, 3]);
        eprintln!("{name} (x Int): {points} boundary points");
    }
}

#[test]
fn the_rules_that_cannot_convert_say_why() {
    assert_eq!(notes("pow_to_powabs", "base_may_be_negative"), [code(104)]);
    assert_eq!(notes("round_half_even_to_away", "draw"), [code(105)]);
    assert_eq!(notes("sgn_to_ternary", "draw"), [code(105)]);
    assert_eq!(notes("pwl_to_ternary", "decreasing"), [code(106)]);
    assert_eq!(notes("pow_signed_to_ternary", "draw"), [code(105)]);
}

#[test]
fn pwl_to_ternary_keeps_every_value_on_drawn_points() {
    // oracle: eval_typed of `pwl` itself, on 1 to 5 strictly increasing literal points (Int
    // and Real forms, negative and fractional), the input bound to the boundary set
    let mut h = 0x2545_F491_4F6C_DD1Du64;
    let mut next = move |n: u64| {
        h ^= h << 13;
        h ^= h >> 7;
        h ^= h << 17;
        h % n
    };
    let mut points = 0;
    for _ in 0..100 {
        let n = 1 + next(5);
        let mut at = next(7) as i64 - 3;
        let mut args = String::new();
        for _ in 0..n {
            let y = next(11) as i64 - 5;
            let form = |v: i64, k: u64| match k {
                0 => format!("{v}i"),
                1 => format!("{v}.5"),
                _ => format!("{v}"),
            };
            args.push_str(&format!(", {}, {}", form(at, next(3)), form(y, next(3))));
            at += 1 + next(3) as i64;
        }
        let text = format!(
            "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = pwl(x{args}) /: 2i\n"
        );
        points += keeps_boundary_values("pwl_to_ternary", &text, ANY);
    }
    eprintln!("pwl_to_ternary: {points} drawn points");
}

#[test]
fn fold_constant_bitops_keeps_every_value() {
    // oracle: eval_typed of the operator itself on integer literals (both signs, the 2⁵² and
    // 2⁵³ edges, shifts across 0..64), against the folded literal
    let ints = [
        "0",
        "1",
        "-1",
        "2",
        "3",
        "-3",
        "5i",
        "-6i",
        "12",
        "255",
        "4503599627370495",
        "4503599627370496",
        "-4503599627370497",
        "9007199254740992",
        "63",
        "64",
        "1.5",
    ];
    let ops = [
        ("&", "fold_constant_bitand"),
        ("|", "fold_constant_bitor"),
        ("^", "fold_constant_bitxor"),
        ("<<", "fold_constant_shl"),
        (">>", "fold_constant_shr"),
    ];
    let mut points = 0;
    for (op, name) in ops {
        for a in ints {
            for b in ints {
                let text =
                    format!("file \"p.sp\" deck\n  param x = 1\n  param r = ({a} {op} {b}) + x\n");
                points += keeps_folded(name, &text);
            }
        }
    }
    for a in ints {
        let text = format!("file \"p.sp\" deck\n  param x = 1\n  param r = ~{a} + x\n");
        points += keeps_folded("fold_constant_bitnot", &text);
    }
    eprintln!("fold_constant_bitops: {points} folded points");
    assert!(points > 1000);
    for (_, name) in ops {
        let hit = |e| matches!(e, ir::Expr::Binary(o, ..) if format!("{o:?}").starts_with("Bit") || matches!(o, ir::BinOp::Shl | ir::BinOp::Shr));
        keeps_generated_values(name, hit, false);
        keeps_generated_values(&format!("{name}_dyn"), hit, false);
    }
}

#[test]
fn the_leaves_take_the_one_value_or_say_why() {
    assert_eq!(notes("tnom_to_env", "plain"), []);
    assert_eq!(notes("tnom_to_env", "no_env"), [code(108)]);
    assert_eq!(notes("tnom_to_env", "two_envs"), [code(108)]);
    assert_eq!(notes("tnom_to_env", "from_here"), [code(108)]);
    assert_eq!(notes("substitute_tstep", "two_transients"), [code(108)]);
    assert_eq!(
        notes("fold_constant_bitand", "not_literal"),
        [code(107), code(107)]
    );
    assert_eq!(
        notes("fold_constant_bitand", "typed_division_left"),
        [code(107)]
    );
}

#[test]
fn the_draw_rules_keep_the_nominal_run_and_say_what_changes() {
    // oracle: eval_typed under bindings whose draw reads its nominal (every Dist is centred on
    // its first argument), over designs with no sampling loop
    let no_loops = ir::FeatureSet::ALL
        .minus(ir::FeatureSet::EMPTY.with(ir::Feature::Item(ir::ItemKind::Loop)));
    let draw = |e| matches!(e, ir::Expr::Draw(..));
    for name in [
        "draws_always_to_sampling",
        "draws_per_reference_to_call_site",
        "sampling_draws_to_nominal",
    ] {
        let n = keeps_values_over(name, draw, true, no_loops);
        eprintln!("{name}: {n} generated roots");
        assert!(n > 0, "{name}");
    }
    assert_eq!(notes("draws_always_to_sampling", "plain"), [code(109)]);
    assert_eq!(
        notes("draws_per_reference_to_call_site", "always"),
        [code(110)]
    );
    assert_eq!(
        notes("sampling_draws_to_nominal", "monte_carlo"),
        [code(110)]
    );
}

#[test]
fn long_literal_to_shortest_keeps_every_value() {
    // oracle: eval_typed of the long literal (its correctly rounded value) against the
    // shortest decimal written in its place, under the boundary set
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = 0.12345678901234567890123 * x + 1.00000000000000000000001e-3 / y\n";
    let points = keeps_boundary_values("long_literal_to_shortest", text, ANY);
    eprintln!("long_literal_to_shortest: {points} boundary points");
    assert_eq!(
        notes("long_literal_to_shortest", "plain"),
        [code(111), code(111)]
    );
}

#[test]
fn inline_funcs_keeps_every_value() {
    // oracle: eval_typed of the call (its arguments bound once, the body in a frame) against
    // the copy with the arguments substituted, at the boundary set and on generated designs
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  func g(u) = u * u - 1\n  func f(a, b) = @g(a) / (b + 2) + a\n  param r = @f(x, y) + @g(y)\n  element behav b1 (n 0) expr=(@f(x, y) * v(n))\n";
    let points = keeps_boundary_values("inline_funcs", text, ANY);
    let call = |e| matches!(e, ir::Expr::UserCall(..));
    let generated = keeps_generated_values("inline_funcs", call, true);
    eprintln!("inline_funcs: {points} boundary points, {generated} generated roots");
    assert!(generated > 0);
    assert_eq!(
        notes("inline_funcs", "refused"),
        [code(113), code(113), code(113)]
    );
    assert_eq!(notes("drop_unused_funcs", "plain"), [code(114)]);
}

#[test]
fn legalize_chains_the_two_roundings_for_a_target_with_neither() {
    // oracle: eval_typed of the input root against the legalized root (A32: a pass runs when a
    // registered pass removes what it requires); the target lacks both roundings, so
    // round_half_even_to_away runs only because round_half_away_to_floor removes its output
    let lacks = [
        ir::Feature::Func(ir::Func::RoundHalfEven),
        ir::Feature::Func(ir::Func::RoundHalfAway),
        ir::Feature::FuncDyn(ir::Func::RoundHalfEven),
        ir::Feature::FuncDyn(ir::Func::RoundHalfAway),
    ];
    let target = lacks.iter().fold(ir::FeatureSet::ALL, |t, &f| {
        t.minus(ir::FeatureSet::EMPTY.with(f))
    });
    let text = "file \"p.sp\" deck\n  param x = 1\n  param y = 1\n  param r = round_half_even(x)\n";
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(text, &mut design.builder(), &mut diags);
    let before = compare::roots(&design);
    spicerack::passes::legalize(&mut design, &target, &mut diags);
    assert!(diags.is_empty(), "nothing refused: {:?}", diags.rows());
    let used = ir::features_used(&design);
    assert!(
        lacks.iter().all(|&f| !used.contains(f)),
        "both roundings are gone"
    );
    let pairs = compare::pairs(&before, &compare::roots(&design));
    let (old, new) = *pairs.last().expect("the root of r");
    assert_ne!(old, new);
    for x in bindings::boundary() {
        let env = bindings::Pair {
            design: &design,
            x: ir::Value::real(x),
            y: ir::Value::real(0.0),
        };
        let (a, b) = (ir::eval(&design, old, &env), ir::eval(&design, new, &env));
        assert_eq!(a.map(f64::to_bits), b.map(f64::to_bits), "x = {x:e}");
    }
}

#[test]
fn legalize_refuses_the_power_cycle_instead_of_looping() {
    // pow_to_powabs requires PowSigned, which pow_signed_to_ternary removes by requiring Pow:
    // for a target with neither, the least fixed point holds neither pass, and the item is
    // refused (U0001) instead of the two undoing each other
    let lacks = [
        ir::Feature::Func(ir::Func::Pow),
        ir::Feature::Func(ir::Func::PowSigned),
    ];
    let target = lacks.iter().fold(ir::FeatureSet::ALL, |t, &f| {
        t.minus(ir::FeatureSet::EMPTY.with(f))
    });
    let mut design = ir::Design::new();
    let mut diags = ir::Diags::new();
    ir::parse(
        "file \"p.sp\" deck\n  param x = 2\n  param r = pow(x, 3)\n",
        &mut design.builder(),
        &mut diags,
    );
    spicerack::passes::legalize(&mut design, &target, &mut diags);
    let codes: Vec<ir::Code> = diags.rows().iter().map(|d| d.code).collect();
    assert_eq!(codes, [ir::diag::UNSUPPORTED], "{:?}", diags.rows());
}
