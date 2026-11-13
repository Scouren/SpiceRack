//! The plots of one deck's two runs, made comparable: the target's vectors given back their
//! source names (the rename map), both narrowed to the vectors a `.pair` file names, and step
//! 5's check that every source vector has its counterpart.
//!
//! Cites: AMEND-A12 A32 (the rename map is the `name_with_letter` notes); `rewrite/SPINE.md` §8a.
//!
//! Contents:
//! - `pub fn hierarchical(plots: Vec<RawData>) -> Vec<RawData>`
//! - `pub fn renamed(plots: Vec<RawData>, map: &HashMap<String, String>) -> Vec<RawData>`
//! - `pub fn only(plots: Vec<RawData>, keep: Option<&BTreeSet<String>>) -> Vec<RawData>`
//! - `pub fn names(a: &[RawData], b: &[RawData], devices: &BTreeSet<String>, axes: &BTreeSet<String>) -> Result<(), String>`

use std::collections::{BTreeSet, HashMap};

use spicerack::result::RawData;

use crate::cmp;

/// `plots` with ngspice's hierarchical device spelling made the path every other simulator
/// writes: a branch current of a device inside an instance is `i(l.xgood1.l1)` on ngspice (the
/// device's letter, then the path), `Xgood1:L1:flow(br)` on VACASK; the leading one-letter
/// segment that repeats the last segment's letter goes.
pub fn hierarchical(mut plots: Vec<RawData>) -> Vec<RawData> {
    for v in plots.iter_mut().flat_map(|p| p.variables.iter_mut()) {
        let n = cmp::normal(&v.name);
        let Some(path) = n.strip_suffix("#branch") else {
            continue;
        };
        let segs: Vec<&str> = path.split('.').collect();
        let letter = |s: &str| s.chars().next().unwrap_or(' ');
        if segs.len() >= 3 && segs[0].len() == 1 && letter(segs[0]) == letter(segs[segs.len() - 1])
        {
            v.name = segs[1..].join(".") + "#branch";
        }
    }
    plots
}

/// `plots` with each vector named under the rename map: every `.`-segment of its normal name
/// (and a branch's device) that the map holds, given back its source name.
pub fn renamed(mut plots: Vec<RawData>, map: &HashMap<String, String>) -> Vec<RawData> {
    if map.is_empty() {
        return plots;
    }
    for v in plots.iter_mut().flat_map(|p| p.variables.iter_mut()) {
        let n = cmp::normal(&v.name);
        let (path, branch) = match n.strip_suffix("#branch") {
            Some(p) => (p.to_string(), "#branch"),
            None => (n, ""),
        };
        let segs: Vec<&str> = path
            .split('.')
            .map(|s| map.get(s).map_or(s, String::as_str))
            .collect();
        v.name = segs.join(".") + branch;
    }
    plots
}

/// `plots` holding only the vectors `keep` names (and each plot's scale), or all of them.
pub fn only(mut plots: Vec<RawData>, keep: Option<&BTreeSet<String>>) -> Vec<RawData> {
    let Some(keep) = keep else {
        return plots;
    };
    for p in &mut plots {
        let first = usize::from(!matches!(cmp::kind(p), "op" | "total"));
        let held: Vec<usize> = (0..p.variables.len())
            .filter(|&i| i < first || keep.contains(&cmp::normal(&p.variables[i].name)))
            .collect();
        p.real_data = held
            .iter()
            .filter_map(|&i| p.real_data.get(i).cloned())
            .collect();
        p.complex_data = held
            .iter()
            .filter_map(|&i| p.complex_data.get(i).cloned())
            .collect();
        p.variables = held.iter().map(|&i| p.variables[i].clone()).collect();
        for (k, v) in p.variables.iter_mut().enumerate() {
            v.index = k;
        }
    }
    plots
}

/// Step 5: every vector of every plot of `a` is in a plot of its kind in `b`, by normal name.
/// A device's internal unknowns (VACASK's `d1:a_int`, `m1:implicit_equation_0`: a path through
/// a name of `devices`) are its simulator's own, not the deck's, and are left out; so are a DC
/// plot's axis vectors named in `axes` (the names the target could not spell).
// ponytail: devices by name across scopes; a device and a subcircuit instance of one name in
// two scopes hide the instance's nodes. Key by scope path when a deck does that.
pub fn names(
    a: &[RawData],
    b: &[RawData],
    devices: &BTreeSet<String>,
    axes: &BTreeSet<String>,
) -> Result<(), String> {
    let mut missing = Vec::new();
    for p in a {
        let k = cmp::kind(p);
        let first = usize::from(!matches!(k, "op" | "total"));
        let theirs: BTreeSet<String> = b
            .iter()
            .filter(|q| cmp::kind(q) == k)
            .flat_map(|q| q.variables.iter().map(|v| cmp::normal(&v.name)))
            .collect();
        for v in p.variables.iter().skip(first) {
            let n = cmp::normal(&v.name);
            let path: Vec<&str> = n.trim_end_matches("#branch").split('.').collect();
            let internal = path[..path.len() - 1].iter().any(|s| devices.contains(*s));
            // A named DC axis's vector (VACASK's `sweep s1` writes `s1`) is the plot's grid,
            // which step 4 pairs; ngspice writes the inner axis as its scale and the outer
            // ones not at all.
            let axis = k == "dc" && axes.contains(&n);
            if !internal && !axis && !theirs.contains(&n) {
                missing.push(format!("{k} {}", v.name));
            }
        }
    }
    match missing.is_empty() {
        true => Ok(()),
        false => Err(format!("no counterpart for {}", missing.join(", "))),
    }
}
