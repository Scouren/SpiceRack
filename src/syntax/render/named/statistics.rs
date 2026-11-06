//! A Spectre `statistics { … }` block (`[REF]` pp.175-179): a deck's own
//! blocks as read, then the Monte Carlo lift's (`lift::Lifted`) for the top
//! or a section, one statement a line.
//!
//! Cites: Spectre Circuit Simulator Reference (`[REF]`) pp.175-179
//! (`statistics`, `process`, `mismatch`, `vary`, `correlate`, `truncate`).
//!
//! Contents:
//! - `W::statistics(&mut self, b: &Block, at: Option<usize>)`, `W::stat_block(&mut self, st: &Stats)`

use super::W;
use crate::syntax::render::read::{Block, Stats};

impl W<'_, '_> {
    /// `statistics { … }`: block `b`'s, then the lift's for the top or
    /// section `at` (`[REF]` pp.175-179), one statement a line.
    pub(super) fn statistics(&mut self, b: &Block, at: Option<usize>) {
        for &st in &b.statistics {
            let s = self.rd.statistics(st);
            self.stat_block(&s);
        }
        if let Some(s) = self.lifted.block_at(at) {
            self.stat_block(&s);
        }
    }

    /// One `statistics { … }` block, `st`.
    pub(super) fn stat_block(&mut self, st: &Stats) {
        self.line("statistics {");
        self.o.indent();
        self.o.indent();
        let scopes = [
            ("process", &st.process, &st.truncate[1]),
            ("mismatch", &st.mismatch, &st.truncate[2]),
        ];
        for (name, varies, t) in scopes {
            if varies.is_empty() && t.is_none() {
                continue;
            }
            self.line(&format!("{name} {{"));
            self.o.indent();
            self.o.indent();
            for x in varies {
                let key = if x.dist == "unif" { "N" } else { "std" };
                let pct = if x.percent { " percent=yes" } else { "" };
                let spread = self.value_text(&x.spread);
                self.line(&format!(
                    "vary {} dist={} {key}={spread}{pct}",
                    x.param, x.dist
                ));
            }
            if let Some(t) = t {
                self.line(&format!("truncate tr={}", self.value_text(t)));
            }
            self.o.dedent();
            self.o.dedent();
            self.line("}");
        }
        for (params, devices, cc) in &st.correlations {
            let mut s = String::from("correlate");
            if !devices.is_empty() {
                s.push_str(&format!(" dev=[{}]", devices.join(" ")));
            }
            if !params.is_empty() {
                s.push_str(&format!(" param=[{}]", params.join(" ")));
            }
            s.push_str(&format!(" cc={}", self.value_text(cc)));
            self.line(&s);
        }
        if let Some(t) = &st.truncate[0] {
            self.line(&format!("truncate tr={}", self.value_text(t)));
        }
        self.o.dedent();
        self.o.dedent();
        self.line("}");
    }
}
