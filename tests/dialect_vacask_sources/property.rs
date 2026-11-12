//! Covering properties (R55) for the kinds the laws' generator does not reach: sources and
//! couplings built through the builder, written by VACASK's writer and read back.
//!
//! Contents: tests only.

use spicerack::ir;

use super::{canonical, read, written};

/// A small deterministic generator (an LCG), so the property is seeded and repeatable.
struct Lcg(u64);

// ---- private ----

impl Lcg {
    /// The next value below `n`.
    fn below(&mut self, n: u64) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 33) % n
    }

    /// A positive literal: plain with up to three decimals, or a scale form.
    fn literal(&mut self) -> ir::Literal {
        let mantissa = 1 + self.below(9999) as i64;
        match self.below(2) {
            0 => ir::Literal {
                mantissa,
                frac: self.below(4) as u8,
                exp10: 0,
                form: ir::NumForm::Plain,
            },
            _ => ir::Literal {
                mantissa,
                frac: 0,
                exp10: [-9i16, -6, -3, 3, 6][self.below(5) as usize],
                form: ir::NumForm::Scale,
            },
        }
    }
}

/// Covering property (R55) for the kinds the laws' generator does not reach (`Wave(Am)`: it
/// draws `mod_offset` freely, VACASK's AM fixes it at 1; `Elem(Coupling)`: one generated in 512
/// seeds): admissible AM sources and mutual couplings built through the builder are written,
/// read back and equal (canonical IR text). Oracle: the round trip (L5) on VACASK's own forms,
/// VD dev-builtin-src (AM), dev-builtin-mutual.
#[test]
fn property_am_and_mutual_round_trip() {
    for seed in 0..64u64 {
        let mut r = Lcg(seed);
        let mut design = ir::Design::new();
        let mut b = design.builder();
        let (_, top) = b.file("prop.sim", ir::Role::Deck);
        let span = ir::Span::NONE;
        b.title(top, "property", span);
        for (name, family) in [
            ("vs", ir::ModelFamily::SourceV),
            ("is", ir::ModelFamily::SourceI),
            ("mu", ir::ModelFamily::Mutual),
        ] {
            let name = b.name(name);
            let none = ir::Ref::unresolved(ir::NameId::NONE);
            b.model(
                top,
                ir::Model {
                    name,
                    params: ir::List::EMPTY,
                    bin_of: ir::NameId::NONE,
                    ako: none,
                    module: none,
                    version: ir::ExprId::NONE,
                    word: ir::NameId::NONE,
                    family,
                    polarity: ir::Polarity::None,
                },
                span,
            );
        }
        for k in 0..4 {
            let mut lit = |b: &mut ir::Builder<'_>| {
                let l = r.literal();
                b.literal(l)
            };
            let one = b.literal(ir::Literal {
                mantissa: 1,
                frac: 0,
                exp10: 0,
                form: ir::NumForm::Plain,
            });
            let am = ir::wave::Am {
                offset: lit(&mut b),
                amplitude: lit(&mut b),
                mod_offset: one,
                mod_index: lit(&mut b),
                mod_freq: lit(&mut b),
                carrier_freq: lit(&mut b),
                delay: lit(&mut b),
                phase_m_deg: lit(&mut b),
                phase_c_deg: lit(&mut b),
                mod_time: ir::wave::AmModTime::Delay,
                pre: ir::wave::PreDelay::HoldDelayValue,
                given_zero: ir::wave::GivenZero::AsGiven,
            };
            let wave = b.wave(am);
            let current = k % 2 == 1;
            let (kind, master) = match current {
                true => (ir::ElemKind::ISource, "is"),
                false => (ir::ElemKind::VSource, "vs"),
            };
            let node = format!("n{k}");
            let head = b.head(
                &format!("s{k}"),
                &[&node, "0"],
                Some(master),
                &[],
                ir::ExprId::NONE,
            );
            let row = ir::source::Source {
                dc: ir::ExprId::NONE,
                ac_mag: ir::ExprId::NONE,
                ac_phase_deg: ir::ExprId::NONE,
                tc1: ir::ExprId::NONE,
                tc2: ir::ExprId::NONE,
                pac_mag: ir::ExprId::NONE,
                pac_phase_deg: ir::ExprId::NONE,
                wave,
                noise_psd: ir::List::EMPTY,
                parasitics: ir::NO_ROW,
                port: ir::NO_ROW,
                distof: ir::NO_ROW,
                dc_rule: ir::source::DcRule::WaveAtZero,
            };
            b.source(top, kind, head, row, span);
            let k_val = lit(&mut b);
            let inductors = b.names(&[&format!("la{k}"), &format!("lb{k}")]);
            let head = b.head(&format!("m{k}"), &[], Some("mu"), &[], ir::ExprId::NONE);
            b.coupling(
                top,
                head,
                ir::passive::Coupling {
                    inductors,
                    k: k_val,
                },
                span,
            );
        }
        let mut diags = ir::Diags::new();
        ir::link(&mut design, &mut diags);
        let before = canonical(&design);
        let (again, _) = read(&written(&mut design), "vacask");
        assert_eq!(canonical(&again), before, "seed {seed}");
    }
}

/// Oracle: A31 and VD dev-builtin-src (AM: `1 + modindex·sin(…)`): an `Am` whose `mod_offset`
/// is not the real literal 1 (`2.`, and `1i`, an Int) is the feature `AmModOffset`, which
/// VACASK lacks, so `legalize` refuses it by that feature (no writer-side refusal).
#[test]
fn an_am_offset_other_than_one_is_a_missing_feature() {
    for offset in ["2.", "1i"] {
        let text = format!(
            "file \"am.sim\" deck\n  title \"am\"\n  model vs source_v\n  element vsource v1 (a 0) model=vs wave=am(offset=0 amplitude=1 mod_offset={offset} mod_index=0.5 mod_freq=1k carrier_freq=1k delay=0 phase_m_deg=0 phase_c_deg=0 pre=hold_delay_value) dc_rule=wave_at_zero\n"
        );
        let (mut design, _) = read(&spicerack::dialect::Sources::one("am.ir", &text), "ir");
        let row = spicerack::dialect::by_name("vacask").expect("the vacask row");
        let mut diags = ir::Diags::new();
        spicerack::dialect::legalize(&mut design, row, &mut diags);
        let want = ir::Feature::Circuit(ir::CircuitExtra::AmModOffset);
        let refused = diags.rows().iter().any(|d| {
            spicerack::dialect::refuses(d.code) && ir::Feature::from_index(d.arg as u16) == want
        });
        assert!(refused, "offset {offset}: {:?}", diags.rows());
    }
}
