//! A Xyce deck's global nodes written for a target where `$` starts a
//! comment.
//!
//! Cites: Xyce RG §2.3.4 (`$G_` nodes); ngspice manual §2.4.4 (`.global`).
//!
//! Contents:
//! - `pub(super) fn xyce_globals(text: &str) -> String`

/// A Xyce deck's global nodes (`$G_name`, Xyce RG §2.3.4: a node every
/// subcircuit sees) as `.global` nodes for ngspice and HSPICE, where `$`
/// starts a comment (ngspice-45 on `VDD $G_VDDNODE 0 5`: "Not enough
/// parameters"): each is renamed `G_name` (another suffix where the deck
/// holds that word) and declared after the title.
pub(super) fn xyce_globals(text: &str) -> String {
    let word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mut found: Vec<&str> = Vec::new();
    for (i, _) in text.match_indices("$G_") {
        let end = text[i + 1..]
            .find(|c: char| !word(c))
            .map_or(text.len(), |k| i + 1 + k);
        if !found.contains(&&text[i..end]) {
            found.push(&text[i..end]);
        }
    }
    if found.is_empty() {
        return text.to_string();
    }
    // A word of the deck that is no `$G_` node's own.
    let taken = |n: &str| {
        text.match_indices(n).any(|(i, _)| {
            let before = text[..i].chars().next_back();
            let after = text[i + n.len()..].chars().next();
            !before.is_some_and(|c| word(c) || c == '$') && !after.is_some_and(word)
        })
    };
    let found_len = found.len();
    let mut names: Vec<String> = Vec::with_capacity(found_len);
    for &g in &found {
        let base = &g[1..];
        let name = std::iter::once(base.to_string())
            .chain((1..).map(|k| format!("{base}_{k}")))
            .find(|n| !taken(n) && !names.contains(n))
            .expect("an unused name");
        names.push(name);
    }
    // The longest node first: `$G_A` is also the head of `$G_AB`, and a
    // replace of the shorter first would rename the longer's head (Xyce
    // 7.10 on `V1 $G_A 0 1`, `R1 $G_AB 0 1k`, `R2 G_AB 0 1k`: three nodes).
    let mut order: Vec<usize> = (0..found_len).collect();
    order.sort_by_key(|&i| std::cmp::Reverse(found[i].len()));
    let mut out = text.to_string();
    for i in order {
        out = out.replace(found[i], &names[i]);
    }
    debug_assert!(!out.contains("$G_"), "every $G_ node renamed");
    debug_assert_eq!(names.len(), found_len);
    let line = format!(".global {}\n", names.join(" "));
    match out.find('\n') {
        Some(i) if out.starts_with('*') => format!("{}{line}{}", &out[..=i], &out[i + 1..]),
        _ => format!("{line}{out}"),
    }
}

#[cfg(test)]
mod tests {
    use super::xyce_globals;

    // Oracle: Xyce 7.10 on this deck keeps three nodes (1 V, 2 V, 3 V with
    // the sources of `tests/test_xyce_source.rs`); the text is derived by
    // hand: `G_AB` is the deck's own word, so `$G_AB` takes `G_AB_1`.
    #[test]
    fn a_global_that_heads_another_keeps_its_own_name() {
        let out = xyce_globals("* t\nV1 $G_A 0 1\nR1 $G_AB 0 1k\nR2 G_AB 0 1k\n");
        assert_eq!(
            out,
            "* t\n.global G_A G_AB_1\nV1 G_A 0 1\nR1 G_AB_1 0 1k\nR2 G_AB 0 1k\n"
        );
    }

    // Two globals never share a name: `$G_X` takes `G_X_1` (the deck holds
    // `G_X`), so `$G_X_1` moves on to `G_X_1_1`.
    #[test]
    fn two_globals_never_take_one_name() {
        let out = xyce_globals("* t\nR1 $G_X 0 1\nR2 $G_X_1 0 1\nR3 G_X 0 1\n");
        assert_eq!(
            out,
            "* t\n.global G_X_1 G_X_1_1\nR1 G_X_1 0 1\nR2 G_X_1_1 0 1\nR3 G_X 0 1\n"
        );
    }
}
