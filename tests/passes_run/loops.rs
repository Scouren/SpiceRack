//! Seeded designs of the shape `loop_end` converts (R55: built for the rule, since the
//! generator's loops are almost never a `.param` over a list): top parameters, one to three
//! loops over them (a list sweep or a table, restoring or keeping the last value, list lengths
//! one to four), analyses inside and after each, and sometimes an alter of an unswept one.
//!
//! Contents: `pub fn design(u64) -> String`

/// One seeded design, as IR text.
pub fn design(seed: u64) -> String {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut next = move |n: u64| {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        x % n
    };
    let params = ["p", "q", "r", "s"];
    let mut t = String::from("file \"a.sp\" deck\n");
    for p in params {
        t += &format!("  param {p} = {}\n", 1 + next(9));
    }
    if next(3) == 0 {
        t += &format!("  alter (param s) value={}\n", 20 + next(9));
    }
    // each loop sweeps a parameter no earlier loop swept (else the rule refuses it)
    let loops = 1 + next(3) as usize;
    for swept in &params[..loops] {
        let end = if next(2) == 0 { " keep_last" } else { "" };
        let n = 1 + next(4);
        let values: Vec<String> = (0..n).map(|_| format!("{}", 10 + next(90))).collect();
        match next(2) {
            0 => {
                t += &format!(
                    "  loop sweep{end} (param {}) (list values=[{}])\n",
                    swept,
                    values.join(", ")
                );
            }
            _ => {
                let rows: Vec<String> = values
                    .iter()
                    .map(|v| format!("{v}, {}", 100 + next(9)))
                    .collect();
                t += &format!(
                    "  loop table{end} (param {}) (param s) values=[{}]\n",
                    swept,
                    rows.join(", ")
                );
            }
        }
        for _ in 0..1 + next(2) {
            t += "    analysis op\n";
        }
        for _ in 0..next(3) {
            t += "  analysis op\n";
        }
    }
    t += "  analysis op\n";
    t
}
