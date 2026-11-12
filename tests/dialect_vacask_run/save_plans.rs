//! The seeded save property (R55): `Item(Save)` is unreached by the shared laws (the neutral
//! generator cannot know the two save shapes VACASK spells), so this builds designs of those
//! shapes from a seed and checks `read(write(x)) ≅ x` (`eq_meaning`).
//!
//! Contents: tests only.

use super::{canonical, read, read_ir, write_entry};

// ---- private ----

/// A deck in IR text from `seed`: up to eight items, each a save of a shape VACASK spells or
/// a named analysis, at least two saves.
fn save_plan(seed: u64) -> String {
    let mut r = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    let mut next = |n: u64| {
        r = r
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (r >> 33) % n
    };
    const PROBES: [&str; 5] = [
        "(v (a))",
        "(v (x1 n))",
        "(i (v1))",
        "(param (r1) (i))",
        "(param (x1 m1) (gm))",
    ];
    const ANALYSES: [&str; 3] = [
        "analysis op name=op{k}",
        "analysis tran name=tr{k} stop=1m cap=span_over50",
        "analysis ac name=ac{k} (list values=[1k])",
    ];
    let mut out = String::from("file \"g.sim\" deck\n  title \"g\"\n");
    let items = 2 + next(7);
    let mut made = 0;
    for k in 0..items {
        let save = made < 2 && items - k <= 2 - made || next(2) == 0;
        if !save {
            let a = ANALYSES[next(3) as usize].replace("{k}", &k.to_string());
            out.push_str(&format!("  {a}\n"));
            continue;
        }
        made += 1;
        // Probes in a random order: every combination of the five, at least one.
        let mut order: Vec<usize> = (0..PROBES.len()).collect();
        for i in (1..order.len()).rev() {
            order.swap(i, next(i as u64 + 1) as usize);
        }
        let default = next(3) == 0;
        let mut probes: Vec<&str> = order
            .iter()
            .filter(|&&i| next(2) == 0 && !(default && PROBES[i].starts_with("(param")))
            .map(|&i| PROBES[i])
            .collect();
        if default {
            let at = next(probes.len() as u64 + 1) as usize;
            probes.insert(at, "(all_v) (all_branch_i)");
        } else if probes.is_empty() {
            probes.push(PROBES[next(PROBES.len() as u64) as usize]);
        }
        let set = match default {
            true => "(op dc ac tran dc_inc)",
            false => "(op dc tran)",
        };
        out.push_str(&format!(
            "  save analysis={set} from_here {}\n",
            probes.join(" ")
        ));
        // A reset of the saves between them (VACASK's `clear saves`, run sv2).
        if next(4) == 0 {
            out.push_str("  reset saves\n");
        }
    }
    out
}

#[test]
fn generated_save_plans_read_back_to_their_meaning() {
    // Oracle: law L5 (`read(write(x)) ≅ x`, `eq_meaning`) on designs this test builds from a
    // seed, holding only the save shapes VACASK spells (R55: the neutral generator cannot know
    // them, so `Item(Save)` is unreached by the laws and this property covers it): the
    // op/dc/tran set with `v`, `i`, `p` probes in every combination and order (a hierarchical
    // one among them), the wider set with `default` and no `p()`, several saves before and
    // between analyses, each holding from where it stands, a reset of the saves between them
    // (runs sv1, sv2). Every design must be admitted; floors keep it from passing on nothing.
    let (mut designs, mut saves, mut resets) = (0, 0, 0);
    for seed in 0..256u64 {
        let text = save_plan(seed);
        let Some((design, _)) = read_ir(&text) else {
            panic!("seed {seed}: the plan does not read:\n{text}");
        };
        let mut legal = design.clone();
        let written = write_entry(&mut legal);
        let (back, _) = read("g.sim", &written, "vacask");
        assert!(
            back.eq_meaning(&design),
            "seed {seed}:\n{text}written as\n{written}read back as\n{}",
            canonical(&back)
        );
        designs += 1;
        saves += text.matches("\n  save ").count();
        resets += text.matches("\n  reset saves").count();
    }
    assert!(
        designs >= 256 && saves >= 512 && resets >= 64,
        "{designs} designs, {saves} saves, {resets} resets"
    );
}
