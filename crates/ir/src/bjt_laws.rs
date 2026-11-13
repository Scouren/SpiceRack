//! The BJT model laws two simulator builds disagree on, as features: a Gummel-Poon model row (and
//! the BJT instance that names it) in, the member of its law out when the laws differ on it.
//! R86: the base-collector saturation current's area law (`BcSatArea`), carried by the instance;
//! R88: the substrate saturation current's temperature law (`SubSatTemp`), carried by the model.
//! Split from `circuit_features.rs` at its size limit.
//!
//! Cites: `crates/ir/src/model.rs` (`BcSatArea`, `SubSatTemp` and their runs).
//!
//! Contents (crate-private): `pub(crate) fn sub_sat_temp(&Design, &Model) -> Option<CircuitExtra>`,
//! `pub(crate) fn bc_sat_area(&Design, &Element, ExprId, &mut impl FnMut(CircuitExtra))`

use crate::circuit_extra::CircuitExtra as X;
use crate::design::Design;
use crate::element::{ElemKind, Element};
use crate::id::ExprId;
use crate::item::ItemKind;
use crate::model::{BcSatArea, Model, ModelFamily, SubSatTemp};

/// R88: a Gummel-Poon model's substrate temperature member, when the two laws differ on it
/// (`CircuitExtra::ModelSubSatLastFactor`'s doc: `iss` given, `tlev` absent or literally 0 or 1,
/// `ns` not literally the last-set factor's exponent). O(the model's parameters).
pub(crate) fn sub_sat_temp(design: &Design, m: &Model) -> Option<X> {
    if m.family != ModelFamily::GummelPoon {
        return None;
    }
    let get = |key: &str| {
        design
            .params(m.params)
            .iter()
            .find(|p| design.str(p.0) == key)
            .map(|p| p.1)
    };
    let tlev_ok =
        get("tlev").is_none_or(|e| literal(design, e).is_some_and(|v| v == 0.0 || v == 1.0));
    get("iss")?;
    if !tlev_ok {
        return None;
    }
    // the exponent the last-set factor uses: nr with both currents, else 1; absent means 1
    let both = get("ibe").is_some() && get("ibc").is_some();
    let other = if both {
        get("nr").map_or(Some(1.0), |e| literal(design, e))
    } else {
        Some(1.0)
    };
    let ns = get("ns").map_or(Some(1.0), |e| literal(design, e));
    if matches!((ns, other), (Some(a), Some(b)) if a == b) {
        return None;
    }
    Some(match m.sub_sat_temp {
        SubSatTemp::LastFactor => X::ModelSubSatLastFactor,
        SubSatTemp::EmissionNs => X::ModelSubSatEmissionNs,
    })
}

/// R86: a BJT's base-collector saturation member, when the two laws differ on it: its area is
/// not literally 1 and its model gives neither `ibe` nor `ibc` (`BcSatArea`). Reads the one
/// model row the instance names (O(the model's parameters)), never another instance. An
/// unresolved model's law is unknown: both members.
pub(crate) fn bc_sat_area(design: &Design, e: &Element, area: ExprId, add: &mut impl FnMut(X)) {
    if e.kind != ElemKind::Bjt || area.is_none() || crate::run_words::is_one(design, area) {
        return;
    }
    let def = e.model.def;
    let Some(d) = def.index_checked(design.defs.len()).map(|k| design.defs[k]) else {
        add(X::BjtBcSatAreaTimesAreab);
        add(X::BjtBcSatAreabOnly);
        return;
    };
    if d.item.kind != ItemKind::Model {
        return;
    }
    let m: Model = design.row(d.item.index);
    let given = |key: &str| {
        design
            .params(m.params)
            .iter()
            .any(|p| design.str(p.0) == key)
    };
    if m.family != ModelFamily::GummelPoon || given("ibe") && given("ibc") {
        return;
    }
    add(match m.bc_sat_area {
        BcSatArea::AreaTimesAreab => X::BjtBcSatAreaTimesAreab,
        BcSatArea::AreabOnly => X::BjtBcSatAreabOnly,
    });
}

// ---- private ----

/// The value of a literal number; `None` for a parameter read or any other expression.
fn literal(design: &Design, e: ExprId) -> Option<f64> {
    match design.expr(e) {
        crate::expr::Expr::Num(k) => Some(design.literal_value(design.num(k))),
        _ => None,
    }
}
