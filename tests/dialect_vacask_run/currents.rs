//! The writer's refusals of an element current VACASK cannot save (W2314, run F13) and of a lin
//! count below one (W2315, run132).
//!
//! Contents: tests only.

use super::read;
use spicerack::dialect;
use spicerack::ir;

// ---- private ----

/// Oracle: settle run/save_current (run F13): a capacitor's current has no directive (`i(c1)`:
/// "Node 'c1:flow(br)' not found.", `p('c1', i)`: "Output variable 'i' of instance 'c1' not
/// found."); a p() line governs op, its sweeps and tran only, not ac and dcinc (run114); a
/// resistor, a vsource and a name no element has are spelled.
#[test]
fn a_current_vacask_cannot_save_is_refused() {
    let head = r#"file "w.ir" deck
  title "currents"
  device_library resistor
  device_library capacitor
  model vs source_v
  model r resistor
  model c capacitor
  element vsource v1 (a 0) model=vs dc=1i
"#;
    let cases: [(&str, bool); 5] = [
        (
            "  element resistor r1 (a 0) model=r value=1k\n  save analysis=(op dc tran) from_here (i (r1))\n",
            false,
        ),
        (
            "  save analysis=(op dc tran) from_here (i (v1)) (i (nobody))\n",
            false,
        ),
        (
            "  element capacitor c1 (a 0) model=c value=1p\n  save analysis=(op dc tran) from_here (i (c1))\n",
            true,
        ),
        // VACASK lacks Run(RunInterleaved): legalize moves the save after the element.
        (
            "  save analysis=(op dc tran) from_here (i (r1))\n  element resistor r1 (a 0) model=r value=1k\n",
            false,
        ),
        (
            "  element resistor r1 (a 0) model=r value=1k\n  save analysis=(op dc tran ac dc_inc) from_here (all_v) (all_branch_i) (i (r1))\n",
            true,
        ),
    ];
    let row = dialect::by_name("vacask").expect("the vacask row");
    for (body, refused) in cases {
        let (mut design, _) = read(
            "w.ir",
            &format!("{head}{body}  analysis op name=op1\n"),
            "ir",
        );
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row, &mut diags);
        let got = diags.rows().iter().any(|d| dialect::refuses(d.code));
        assert_eq!(got, refused, "{body}{:?}", diags.rows());
    }
}

/// Oracle: run132 (scratchpad vacask/run132): VACASK refuses a negative lin interval count ("Number
/// of intervals (specified by points parameter) must be nonnegative."), so an IR count below 1
/// is refused (W2315); a count of 1 is `points=0`, one frequency. Law L5 seed 3 (the IR round):
/// -424 was written `points=-425`.
#[test]
fn a_lin_count_below_one_is_refused() {
    let row = dialect::by_name("vacask").expect("the vacask row");
    for (points, refused) in [("-424i", true), ("0i", true), ("1i", false), ("3i", false)] {
        let text = format!(
            "file \"w.ir\" deck\n  title \"counts\"\n  analysis ac name=a1 (lin start=1k stop=2k points={points})\n"
        );
        let (mut design, _) = read("w.ir", &text, "ir");
        let mut diags = ir::Diags::new();
        dialect::legalize(&mut design, row, &mut diags);
        let got: Vec<String> = diags
            .rows()
            .iter()
            .filter(|d| dialect::refuses(d.code))
            .map(|d| format!("{}{:04}", d.code.letter as char, d.code.number))
            .collect();
        let want: Vec<String> = match refused {
            true => vec!["W2315".to_string()],
            false => Vec::new(),
        };
        assert_eq!(got, want, "{points}");
    }
}
