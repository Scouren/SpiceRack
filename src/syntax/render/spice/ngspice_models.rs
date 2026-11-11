//! A Xyce `.model` card → the card ngspice's, ESPice's or HSPICE's device
//! of the same equations reads: its level as the target numbers it, or the
//! code that refuses a level the target has no device for. ngspice-45 does
//! not stop at such a level: above 99 it warns "Level must be < 99
//! (Setting Level to 1)" and computes level 1 (`inpfindl.c:51-57`), so a
//! copied Xyce level is a silent wrong number (Xyce 7.10 PSP `level=103`
//! at Vgs = Vds = 1 V: 1.111 mA; ngspice-45 on the copy: 0.100 mA).
//!
//! Adding a level: one `(Xyce, target)` pair in the kind's row of the
//! target's table, with the run that shows the two devices agree. A kind
//! with no row is copied.
//!
//! Cites: ngspice-45 `spicelib/parser/inpdomod.c` (the level switch; its
//! own lists at L73-75 and L383-385), `inpfindl.c:51-57`; Xyce RG 7.10 `M`
//! and `Q` level tables; HSPICE Reference Manual: MOSFET Models D-2010.12
//! Table 1 (p.9-12).
//!
//! Contents:
//! - `pub(super) fn ngspice_model(hspice: bool, kind: &str, params: &[(String, String)]) -> Result<Option<Vec<(String, String)>>, Code>`

use crate::syntax::diag::Code;
use crate::syntax::render::models::fold;

/// One device family: its `.model` kinds, and each Xyce level with the
/// target's level of the same equations. A level with no pair has no
/// device in the target.
struct Levels {
    kinds: &'static [&'static str],
    same: &'static [(u32, u32)],
}

/// ngspice-45 and ESPice. MOS 1, 2, 3, 6 and BJT 1 are SPICE3's in both;
/// 49 is the reader's level for Xyce's BSIM3 (9); 14 and 54 are BSIM4;
/// Xyce's BSIM-SOI 4 (70, and 70450 its 4.5.0) is ngspice's B4SOI 58 (4.4,
/// `b4soiset.c:123`; run: 2.264 mA against 2.252 mA on the corpus card
/// `B4SOI_461/invert_b4soi.cir`); HICUM/L2 234 is ngspice's 8 (both
/// v2.4.0; run: v(c) 0.540334 against 0.540309). Xyce's 10 is BSIM-SOI
/// 3.2 and ngspice's 10 is B4SOI: no pair.
const NGSPICE: &[Levels] = &[
    Levels {
        kinds: &["nmos", "pmos"],
        same: &[
            (1, 1),
            (2, 2),
            (3, 3),
            (6, 6),
            (14, 14),
            (49, 49),
            (54, 54),
            (70, 58),
            (70450, 58),
        ],
    },
    Levels {
        kinds: &["npn", "pnp"],
        same: &[(1, 1), (234, 8)],
    },
];

/// HSPICE (MOSFET Models Table 1): 1, 2, 3, 49 (BSIM3v3) and 54 (BSIM4)
/// are Xyce's; its 6 is Lattin-Jenkins-Grove and its 14 Siemens' model,
/// so Xyce's BSIM4 14 is 54 and Xyce's 6 has no pair.
const HSPICE: &[Levels] = &[
    Levels {
        kinds: &["nmos", "pmos"],
        same: &[(1, 1), (2, 2), (3, 3), (14, 54), (49, 49), (54, 54)],
    },
    Levels {
        kinds: &["npn", "pnp"],
        same: &[(1, 1)],
    },
];

/// The card of a Xyce `.model x <kind>` with `params` as ngspice and
/// ESPice read it, or as HSPICE does (`hspice`): `Ok(None)` unchanged (no
/// `level`, the same number, or a kind with no row), `Ok(Some(..))` its
/// level the target's.
///
/// # Errors
/// W0104: a level the target has no device of the same equations for, or
/// one that is not a number.
pub(super) fn ngspice_model(
    hspice: bool,
    kind: &str,
    params: &[(String, String)],
) -> Result<Option<Vec<(String, String)>>, Code> {
    let table = if hspice { HSPICE } else { NGSPICE };
    let kind = kind.to_ascii_lowercase();
    let Some(row) = table.iter().find(|r| r.kinds.contains(&kind.as_str())) else {
        return Ok(None);
    };
    debug_assert!(!row.same.is_empty(), "{kind}: a family with no level");
    let mut out = params.to_vec();
    let mut changed = false;
    for (k, v) in &mut out {
        if !k.eq_ignore_ascii_case("level") {
            continue;
        }
        let level = fold(v).ok_or(Code::W0104)?;
        let &(_, to) = row
            .same
            .iter()
            .find(|(xyce, _)| f64::from(*xyce) == level)
            .ok_or(Code::W0104)?;
        // The same number keeps the card's spelling (`LEVEL=54.0`).
        if f64::from(to) != level {
            *v = to.to_string();
            changed = true;
        }
    }
    debug_assert_eq!(out.len(), params.len());
    Ok(changed.then_some(out))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(level: &str) -> Vec<(String, String)> {
        vec![
            ("LEVEL".to_string(), level.to_string()),
            ("vth0".to_string(), "0.4".to_string()),
        ]
    }

    // Oracle: ngspice-45 `inpdomod.c` L383-385 ("Only MOS device levels
    // 1-6,8-10,14,49,54-58,60,68,73 are supported in this binary") and
    // L73-75 ("Only BJT levels 1-2, 4, 8, 9 are supported in this binary").
    #[test]
    fn an_ngspice_level_is_one_inpdomod_lists() {
        let has: [&[u32]; 2] = [
            &[
                1, 2, 3, 4, 5, 6, 8, 9, 10, 14, 49, 54, 55, 56, 57, 58, 60, 68, 73,
            ],
            &[1, 2, 4, 8, 9],
        ];
        assert_eq!(NGSPICE.len(), has.len());
        for (row, has) in NGSPICE.iter().zip(has) {
            for (_, to) in row.same {
                assert!(has.contains(to), "{:?}: {to}", row.kinds);
            }
        }
    }

    // Oracle: the runs in the tables' comments (`research/impl/evidence/P3-006`).
    #[test]
    fn a_level_is_the_targets_or_refused() {
        assert_eq!(ngspice_model(false, "NMOS", &card("103")), Err(Code::W0104));
        assert_eq!(ngspice_model(false, "nmos", &card("10")), Err(Code::W0104));
        assert_eq!(
            ngspice_model(false, "nmos", &card("{lv}")),
            Err(Code::W0104)
        );
        assert_eq!(ngspice_model(false, "nmos", &card("54.0")), Ok(None));
        assert_eq!(
            ngspice_model(false, "npn", &card("234")),
            Ok(Some(card("8")))
        );
        assert_eq!(ngspice_model(true, "npn", &card("234")), Err(Code::W0104));
        assert_eq!(
            ngspice_model(true, "nmos", &card("14")),
            Ok(Some(card("54")))
        );
        assert_eq!(ngspice_model(true, "nmos", &card("6")), Err(Code::W0104));
        assert_eq!(ngspice_model(false, "d", &card("200")), Ok(None));
        assert_eq!(ngspice_model(false, "npn", &[]), Ok(None));
    }
}
