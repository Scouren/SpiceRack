//! Stand-ins for AMEND-A12 members (rulings R16–R26) that have not landed in `crates/ir`.
//! Each names the member that replaces it; its marker test (`#[should_panic(expected =
//! "A12")]`) goes red (an unexpected pass) the day the member lands, which is when the stand-in
//! goes. Never a conversion: a stand-in states the nearest meaning the IR has today, the same way
//! in the reader and the writer, so law L5 holds meanwhile.
//!
//! Cites: `rewrite/AMEND-A12.md` K1 (addendum `SgnZeroPos`).
//!
//! Contents: `SGN_ZERO_POS` (crate-private const)

use crate::ir;

/// A12 K1 addendum: `Func::SgnZeroPos`, VACASK's `sgn` (−1 below zero, else +1, typed as its
/// operand: run f4). Until it lands `expr_read` reads `sgn(x)` as the sugar `x >= 0 ? 1 : -1`.
pub(super) const SGN_ZERO_POS: Option<ir::Func> = None;

#[cfg(test)]
mod tests {
    use super::*;

    // Marker: passes (it panics) while the member is missing, fails the day the stand-in goes.

    #[test]
    #[should_panic(expected = "A12 K1")]
    fn sgn_is_a_member_typed_as_its_operand() {
        assert!(
            SGN_ZERO_POS.is_some(),
            "A12 K1: Func::SgnZeroPos not landed"
        );
    }
}
