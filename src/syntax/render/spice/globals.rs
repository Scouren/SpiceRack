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
    let mut out = text.to_string();
    let found_len = found.len();
    let mut names = Vec::with_capacity(found_len);
    for g in found {
        let base = &g[1..];
        let name = std::iter::once(base.to_string())
            .chain((1..).map(|k| format!("{base}_{k}")))
            .find(|n| !taken(n))
            .expect("an unused name");
        out = out.replace(g, &name);
        names.push(name);
    }
    debug_assert!(!out.contains("$G_"), "every $G_ node renamed");
    debug_assert_eq!(names.len(), found_len);
    let line = format!(".global {}\n", names.join(" "));
    match out.find('\n') {
        Some(i) if out.starts_with('*') => format!("{}{line}{}", &out[..=i], &out[i + 1..]),
        _ => format!("{line}{out}"),
    }
}
