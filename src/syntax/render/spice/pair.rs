//! A source dialect's behaviours and a target's → the pair rules the
//! SPICE-family writer applies between them: one named fact per rule,
//! decided once per deck ([`Pair::of`]) and read where the rule writes.
//! A pair rule is a meaning the source's simulator gives a deck that the
//! target's would give another way (Xyce's model `R` a multiplier, its
//! `log` base 10, HSPICE's 25 °C), so it is a fact of the two rows
//! together, never of either alone; a rule of the target alone reads
//! `W::b`, of the source alone `Rd::b`.
//!
//! Invariant: a field reads `Behaviours` only (the two dialects' declared
//! facts), never a dialect's identity, so a plugin dialect gets the rule
//! from what it declares; a rule with no declared fact behind it first
//! gets a def row (`syntax::def`, `syntax::dialects`) and then a field.
//!
//! Adding a pair rule: one field here, documented with its evidence (the
//! manual clause or the run), one line in [`Pair::of`], and the read of
//! `self.pair.<field>` at the one site that writes it. An expression
//! rewrite is also a row of `values::REWRITES`.
//!
//! Cites: each field's own clause.
//!
//! Contents:
//! - `pub(super) struct Pair`
//! - `Pair::of(from: &Behaviours, to: &Behaviours) -> Pair`

use crate::syntax::behaviour::{
    AnalysisForm, Behaviours, CardRules, ControlBlock, ExprEval, ModelSemantics, NetlistRules,
    ParamForm, Parasitics, Writer,
};

/// The pair rules between one source and one target. All false for a deck
/// written in its own dialect.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) struct Pair {
    /// Source and target read different text (ESPice reads ngspice's, so
    /// ngspice → ESPice is not): an expression is respelled (the
    /// rewrites below hold only then), unparsed text is refused (W0032).
    pub(super) foreign_text: bool,
    /// Xyce applies a device `IC=` in its operating point (RG `C`, `L`,
    /// `Q`, `M`), ngspice only under UIC (manual §3.3.5): an `.ic` instead
    /// (`initial`), unless the deck's transient has UIC.
    pub(super) device_ic_in_op: bool,
    /// HSPICE and Xyce evaluate a statistical call to its nominal outside
    /// a Monte Carlo run, ngspice draws one (`ternary::nominal`), unless
    /// the deck runs Monte Carlo.
    pub(super) stat_nominal: bool,
    /// Xyce's `$G_` global nodes, where `$` starts a comment (ngspice-45:
    /// "Not enough parameters"): `.global` nodes (`globals`).
    pub(super) xyce_globals: bool,
    /// HSPICE reads a double-quoted value as an expression, the target as
    /// a string: braced.
    pub(super) quoted_expr: bool,
    /// Xyce's `M` on an `X` line multiplies even where the subcircuit
    /// declares `m`, ngspice's only where it does not (RG `X`;
    /// ISSUE_267/test_a3.cir): the declared `m` is renamed.
    pub(super) xyce_m_rename: bool,
    /// Xyce sets a parameter an `X` line passes even where the `.subckt`
    /// line does not declare it (7.10 runs of BUG_1558/bug_1558_3.cir),
    /// ngspice stops on the line ("0 formal but 1 actual params"): the
    /// head declares it (`values::W::undeclared`).
    pub(super) xyce_undeclared: bool,
    /// Xyce's R model `R` multiplies the instance's 1k (RG 7.10 resistor),
    /// the source's is the resistance (ngspice manual §3.3.2): the instance
    /// takes it, the model card drops it.
    pub(super) r_model_multiplies: bool,
    /// ngspice has no `if()` (Xyce's, LTspice's): a ternary.
    pub(super) spell_if: bool,
    /// Xyce and LTspice read a `{…}` group inside an expression as a
    /// parenthesised one; ngspice-45's B-source reader drops the braces
    /// (`{a-b}*i(v1)` is `a-b*i(v1)`: 2.01 for -0.01) and its `.param`
    /// refuses a nested one: parentheses (`ternary::brace_groups`).
    pub(super) brace_groups: bool,
    /// `log` is natural in the source, base 10 on Xyce (RG §2.3): `ln`.
    pub(super) log_to_ln: bool,
    /// Xyce's `log` (base 10) where the target's is natural: `log10`.
    pub(super) log_to_log10: bool,
    /// Xyce's and LTspice's `limit(x, lo, hi)` is a clamp, ngspice's a
    /// distribution: `min(max(…))`.
    pub(super) spell_clamp: bool,
    /// ngspice's `nint` rounds half to even, Xyce's away from zero: the
    /// source's rounding spelled for Xyce.
    pub(super) nint_to_xyce: bool,
    /// Xyce's `nint` spelled for ngspice.
    pub(super) nint_to_ngspice: bool,
    /// An LTspice element's parasitics (`Rser=`, `Cpar=`) where the
    /// target's C and L take none: explicit elements (`parasitics`).
    pub(super) lt_parasitics: bool,
    /// An ngspice SFFM is (VO VA FM MDI FC), SPICE3's, HSPICE's, Xyce's
    /// and LTspice's (VO VA FC MDI FS) (`vsrcload.c`): the two swapped.
    pub(super) sffm_swap: bool,
    /// SPICE3's SFFM on ngspice's cards (ngspice, ESPice, CDL), whose SFFM
    /// limits MDI to FC/FM and is 0 at time zero (ngspice-45
    /// `vsrcload.c:256,272`): a wave past that is a behavioural source of
    /// time (`Wave::ngspice_sffm`, `waves::wave_as_b`).
    pub(super) sffm_as_b: bool,
    /// LTspice's SINE cycle count (help, Current Source: the sine holds
    /// its value after Ncycles/Freq) where the target's SIN never stops
    /// (ngspice manual §4.1.2; Xyce RG; HSPICE UG): a behavioural source
    /// of time (`Wave::sin_cycles`, `waves::wave_as_b`).
    pub(super) sin_cycles_as_b: bool,
    /// A PULSE's cycle count (ngspice's NP, manual §4.1.1; LTspice's
    /// Ncycles) where the target's PULSE has seven values (Xyce 7.10
    /// ignores an eighth, run: 1 V at 12 µs where ngspice-45 and LTspice
    /// give V1; HSPICE UG, PULSE): the pulses as a PWL (`Wave::pulses`).
    /// ESPice runs NP as ngspice does (0.3.0, run); CDL's cards are
    /// ngspice's.
    pub(super) pulse_as_pwl: bool,
    /// Another dialect's `.print` on ngspice's `.print`: each output spelled as
    /// ngspice's `.print` reads it (`prints`).
    pub(super) foreign_prints: bool,
    /// Another dialect's `.meas` on ngspice: one ngspice-45 cannot read is
    /// refused (W0048).
    pub(super) foreign_measures: bool,
    /// `^` is power in one and XOR (or nothing) in the other.
    pub(super) caret_reread: bool,
    /// An ngspice `{}` then-branch ternary on a target that groups it as
    /// C (W0034, W0035).
    pub(super) ngspice_ternaries: bool,
    /// An HSPICE deck elsewhere: its 25 °C TNOM and TEMP written
    /// (`ngspice_options`); its `SEARCH` refused.
    pub(super) hspice_elsewhere: bool,
    /// An HSPICE deck on ngspice: GSHUNT as RSHUNT, GMINDC refused.
    pub(super) hspice_options_on_ngspice: bool,
    /// Another dialect's parameters on Xyce: a bare name braced, a measure's
    /// `VAL=` respelled (`xyce_cards`).
    pub(super) xyce_from_other: bool,
    /// An ngspice deck's model cards on Xyce: each judged by Xyce's device
    /// (`xyce_cards::xyce_instance`).
    pub(super) xyce_from_ngspice: bool,
    /// A Xyce deck's model levels are Xyce's numbering (RG `M`, `Q` level
    /// tables), which ngspice, ESPice and HSPICE number otherwise or lack
    /// (ngspice-45 `inpdomod.c`; above 99 it computes level 1,
    /// `inpfindl.c:51-57`): each card as the target's device reads it, or
    /// refused (`ngspice_models`).
    pub(super) xyce_models_on_ngspice: bool,
    /// Xyce evaluates a function on complex numbers and keeps the real
    /// part, and clamps `atanh` (7.10: `sqrt(-4)` is 0, `acosh(-2.5)`
    /// 1.5668, `atanh(-2.5)` -14.1621), where ngspice-45 fails the
    /// analysis: the domain spelled (`ternary::xyce_domains`).
    pub(super) xyce_domains: bool,
    /// ngspice's `**` and `^` take the base's magnitude (ngspice-45:
    /// `(-2)**3` is 8 in `{}` and in a B source; manual §5.1), the
    /// target's keep a negative base's sign: `abs(x)**y`
    /// (`powers::Law::Abs`), ngspice's power calls spelled as powers first.
    pub(super) abs_power_out: bool,
    /// The reverse, another dialect's power on ngspice: a negative base
    /// keeps its sign under a whole exponent (Xyce 7.10 and LTspice 26.1.1:
    /// `(-2)**3` is -8; HSPICE UG Table 27 `x**y`; Spectre 25.1,
    /// settle-remote): `(sgn(x)*abs(x)**n)` (`powers::Law::Signed`).
    pub(super) abs_power_in: bool,
    /// A Xyce power on ngspice: under an exponent that is no whole
    /// literal Xyce gives the real part of the complex power (7.10:
    /// `(-2.5)**2.1` is 6.5145), spelled with a cosine
    /// (`powers::Law::Real`); its `pow`, `pwr`, `pwrs` spelled as powers.
    pub(super) xyce_power: bool,
    /// Another dialect's deck on LTspice: its SCALE option refused (LTspice
    /// has none and takes every L and W as written) (`ngspice_options`).
    pub(super) ltspice_from_other: bool,
    /// Another dialect's deck on HSPICE: its one GMIN written as GMIN (the
    /// transient's) and GMINDC (the DC one) (`ngspice_options`).
    pub(super) hspice_from_other: bool,
}

impl Pair {
    /// The pair rules from a deck read with `from`'s behaviours to one
    /// written with `to`'s.
    pub(super) fn of(from: &Behaviours, to: &Behaviours) -> Pair {
        let ng = |r: NetlistRules| r == NetlistRules::Ngspice;
        let xy = |r: NetlistRules| r == NetlistRules::Xyce;
        let hs = |r: NetlistRules| r == NetlistRules::Hspice;
        let lt = |r: NetlistRules| r == NetlistRules::Ltspice;
        let (nf, nt) = (from.netlist_rules, to.netlist_rules);
        let (ef, et) = (from.expr_eval, to.expr_eval);
        let foreign = from.text_family != to.text_family;
        let counts = |r: NetlistRules| matches!(r, NetlistRules::Ngspice | NetlistRules::Ltspice);
        let natural =
            |x: ExprEval| matches!(x, ExprEval::Ngspice | ExprEval::Hspice | ExprEval::Ltspice);
        let pair = Pair {
            foreign_text: foreign,
            device_ic_in_op: xy(nf) && ng(nt),
            stat_nominal: matches!(ef, ExprEval::Hspice | ExprEval::Xyce)
                && et == ExprEval::Ngspice,
            xyce_globals: xy(nf) && (ng(nt) || hs(nt)),
            quoted_expr: ef == ExprEval::Hspice && et != ExprEval::Hspice,
            xyce_m_rename: xy(nf) && ng(nt) && to.subckt_multiplicity,
            xyce_undeclared: xy(nf) && ng(nt),
            r_model_multiplies: xy(nt) && !xy(nf),
            spell_if: foreign && et == ExprEval::Ngspice,
            brace_groups: foreign && et == ExprEval::Ngspice,
            log_to_ln: foreign && natural(ef) && et == ExprEval::Xyce,
            log_to_log10: foreign && ef == ExprEval::Xyce && natural(et),
            spell_clamp: foreign
                && matches!(ef, ExprEval::Xyce | ExprEval::Ltspice)
                && et == ExprEval::Ngspice,
            nint_to_xyce: foreign && ef == ExprEval::Ngspice && et == ExprEval::Xyce,
            nint_to_ngspice: foreign && ef == ExprEval::Xyce && et == ExprEval::Ngspice,
            lt_parasitics: from.card_rules == CardRules::Ltspice
                && to.parasitics == Parasitics::Elements,
            sffm_swap: ng(nf) != (ng(nt) || to.writer == Writer::Cdl),
            sffm_as_b: !ng(nf) && (ng(nt) || to.writer == Writer::Cdl),
            sin_cycles_as_b: nf == NetlistRules::Ltspice && nt != NetlistRules::Ltspice,
            pulse_as_pwl: counts(nf) && !(counts(nt) || to.writer == Writer::Cdl),
            foreign_prints: !ng(nf) && to.analysis_form == AnalysisForm::Ngspice,
            foreign_measures: !ng(nf) && ng(nt),
            caret_reread: from.caret_power != to.caret_power,
            ngspice_ternaries: ef == ExprEval::Ngspice && et != ExprEval::Ngspice,
            hspice_elsewhere: hs(nf) && !hs(nt),
            hspice_options_on_ngspice: hs(nf) && to.control_block == ControlBlock::Ngspice,
            xyce_from_other: to.param_form == ParamForm::Xyce && from.param_form != ParamForm::Xyce,
            xyce_from_ngspice: to.param_form == ParamForm::Xyce
                && from.model_semantics == ModelSemantics::Ngspice,
            xyce_models_on_ngspice: xy(nf) && (ng(nt) || hs(nt)),
            xyce_domains: foreign && ef == ExprEval::Xyce && et == ExprEval::Ngspice,
            abs_power_out: foreign && ef == ExprEval::Ngspice && et != ExprEval::Ngspice,
            abs_power_in: foreign && ef != ExprEval::Ngspice && et == ExprEval::Ngspice,
            xyce_power: foreign && ef == ExprEval::Xyce && et == ExprEval::Ngspice,
            ltspice_from_other: !lt(nf) && lt(nt),
            hspice_from_other: !hs(nf) && hs(nt),
        };
        // The two directions of one rule never hold together.
        assert!(!(pair.log_to_ln && pair.log_to_log10));
        assert!(!(pair.nint_to_xyce && pair.nint_to_ngspice));
        assert!(!(pair.abs_power_out && pair.abs_power_in));
        assert!(!pair.xyce_power || pair.abs_power_in);
        if from == to {
            debug_assert_eq!(
                pair,
                Pair::default(),
                "a dialect has no rule against itself"
            );
        }
        pair
    }
}

#[cfg(test)]
mod tests {
    use super::Pair;
    use crate::syntax::{Dialect, Lang};

    #[test]
    fn a_dialect_has_no_rule_against_itself() {
        // A dialect with no reader (CDL, Verilog) is never a source.
        let read = Dialect::ALL
            .into_iter()
            .filter(|&d| Lang::from(d).lexicon().is_some());
        for d in read {
            let b = Lang::from(d).behaviours();
            assert_eq!(Pair::of(b, b), Pair::default(), "{d:?}");
        }
    }

    // Oracle: the clauses each field cites (Xyce RG 7.10, ngspice manual).
    #[test]
    fn xyce_and_ngspice_take_each_others_rules() {
        let (ng, xy) = (Lang::NGSPICE.behaviours(), Lang::XYCE.behaviours());
        let to_ng = Pair::of(xy, ng);
        for (rule, holds) in [
            ("device_ic_in_op", to_ng.device_ic_in_op),
            ("xyce_globals", to_ng.xyce_globals),
            ("xyce_m_rename", to_ng.xyce_m_rename),
            ("xyce_undeclared", to_ng.xyce_undeclared),
            ("log_to_log10", to_ng.log_to_log10),
            ("nint_to_ngspice", to_ng.nint_to_ngspice),
            ("spell_clamp", to_ng.spell_clamp),
            ("stat_nominal", to_ng.stat_nominal),
            ("sffm_swap", to_ng.sffm_swap),
            ("sffm_as_b", to_ng.sffm_as_b),
            ("foreign_measures", to_ng.foreign_measures),
            ("xyce_models_on_ngspice", to_ng.xyce_models_on_ngspice),
            ("brace_groups", to_ng.brace_groups),
            ("xyce_domains", to_ng.xyce_domains),
            ("abs_power_in", to_ng.abs_power_in),
            ("xyce_power", to_ng.xyce_power),
        ] {
            assert!(holds, "xyce -> ngspice: {rule}");
        }
        assert!(!to_ng.r_model_multiplies);
        let to_xy = Pair::of(ng, xy);
        for (rule, holds) in [
            ("r_model_multiplies", to_xy.r_model_multiplies),
            ("log_to_ln", to_xy.log_to_ln),
            ("nint_to_xyce", to_xy.nint_to_xyce),
            ("xyce_from_ngspice", to_xy.xyce_from_ngspice),
            ("xyce_from_other", to_xy.xyce_from_other),
            ("sffm_swap", to_xy.sffm_swap),
            ("abs_power_out", to_xy.abs_power_out),
            ("pulse_as_pwl", to_xy.pulse_as_pwl),
        ] {
            assert!(holds, "ngspice -> xyce: {rule}");
        }
        assert!(!to_xy.device_ic_in_op);
    }
}
