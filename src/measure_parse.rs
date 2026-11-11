//! Parsers for `.meas` results from simulator stdout/log output.
//!
//! Each simulator prints measurement results in a different format:
//!
//! **NGSpice** (stdout, batch mode):
//! ```text
//! rise_time        =  2.345000e-09
//! fall_time        =  1.987000e-09
//! ```
//!
//! A `param` measure with no value is `name = failed`. Any other measure
//! that fails prints no line at all (ngspice names it on stderr), so a
//! card of the deck with no line is a failed measure ([`in_deck_order`]).
//!
//! **LTspice** (log file):
//! ```text
//! Measurement: rise_time
//!   rise_time: AVG=2.345e-09
//! ```
//! Or single-line: `rise_time: 2.345e-009 ...`, `vmax: MAX(V(out))=0.5 FROM 0 TO 3e-06`,
//! `thalf: V(out)=0.3 AT 6e-07` (a WHEN: the point after `AT`), `trf=6e-07 FROM 2e-07 TO 8e-07`
//! (a TRIG/TARG), `Measurement "tnever" FAIL'ed`. With `.step`, a table per measure:
//! ```text
//! Measurement: thalf
//!   step    V(out)=0.3
//!      1    6e-07
//!      2    failed
//! ```
//!
//! A `.four` table is in the same text: ngspice's `Fourier analysis for
//! v(out):` block on stdout, LTspice's `Fourier components of v(out)` block
//! in its log.
//!
//! Simulator text in, [`MeasureResult`] and [`Fourier`] rows out.
//!
//! Cites: none (the formats above are the simulators' printed output:
//! ngspice-45 `src/frontend/fourier.c`, LTspice 26.1.1's log).
//!
//! Contents:
//! - `pub struct Fourier`
//! - `pub fn parse_measures(text: &str, backend: Option<Dialect>) -> Vec<MeasureResult>`
//! - `pub fn in_deck_order(deck: &str, rows: Vec<MeasureResult>, backend: Dialect) -> Vec<MeasureResult>`
//! - `pub fn at_step(rows: &[MeasureResult], k: usize) -> Vec<MeasureResult>`
//! - `pub fn parse_fourier(text: &str) -> Vec<Fourier>`

use crate::result::{MeasureResult, MeasureStatus};
use crate::syntax::Dialect;

/// One output's `.four` table, by column: row `n` of every column is
/// harmonic `n` of the fundamental, row 0 the DC component (LTspice prints
/// that one on its own line: it is row 0 here, with zeros beside it as
/// ngspice prints its own).
#[derive(Debug, Clone, Default)]
pub struct Fourier {
    /// The output as the simulator printed it (`v(out)`).
    pub output: String,
    /// Total harmonic distortion in percent; NaN when the table has no THD
    /// line.
    pub thd: f64,
    /// Hz: `n` times the fundamental.
    pub frequency: Vec<f64>,
    /// The harmonic's amplitude, in the output's unit.
    pub magnitude: Vec<f64>,
    /// Degrees, in the simulator's own reference: for a `SIN` source's
    /// fundamental ngspice prints 0, LTspice 90.
    pub phase: Vec<f64>,
    /// `magnitude` over the fundamental's.
    pub norm_magnitude: Vec<f64>,
    /// `phase` less the fundamental's, in degrees.
    pub norm_phase: Vec<f64>,
}

/// Prefixes of the LTspice log lines that are no measure result: a line
/// with a `:` this table does not name is read as `name: value`. A new
/// log line LTspice writes is a row here.
const LTSPICE_NOT_MEASURES: [&str; 5] = [
    "Circuit:",
    "Date:",
    "Total elapsed",
    ".step",
    // The `.options` echo (`Options:plotwinsize=0 numdgt=15`).
    "Options:",
];

/// The analysis words a `.meas` card names before its measure's name
/// (ngspice: tran, ac, dc, sp; LTspice adds op, tf, noise and lets the
/// word go).
const MEAS_ANALYSES: [&str; 7] = ["tran", "ac", "dc", "op", "sp", "tf", "noise"];

/// Parse measure results from `backend`'s output, in its format (a measure
/// the output says failed is a row too, [`MeasureStatus::Failed`]); with no
/// backend (or one whose format is not known here), whichever format finds
/// results, ngspice's first.
pub fn parse_measures(text: &str, backend: Option<Dialect>) -> Vec<MeasureResult> {
    match backend {
        // ESPice prints `.meas` results in ngspice's format (its output/measure.zig
        // ports com_measure2.c).
        Some(Dialect::Ngspice | Dialect::Espice) => parse_ngspice(text),
        Some(Dialect::Ltspice) => parse_ltspice(text),
        None
        | Some(
            Dialect::Spectre
            | Dialect::Vacask
            | Dialect::Hspice
            | Dialect::Xyce
            | Dialect::Cdl
            | Dialect::Verilog,
        ) => {
            // Try all parsers, return whichever finds results
            let results = parse_ngspice(text);
            if !results.is_empty() {
                return results;
            }
            parse_ltspice(text)
        }
    }
}

/// A run's measures in the order of `deck`'s `.meas` cards (`deck` is the
/// text the simulator ran): each row of `rows` (the printout's,
/// [`parse_measures`]) gets its card's analysis word, a stepped measure's
/// rows staying in step order. ngspice and ESPice print a value or nothing,
/// so on them a card with no row is a failed measure; LTspice names its
/// failures itself. Rows of no card (a `meas` command of a `.control`
/// block) follow, in their order. Of two cards of one name the first
/// takes the rows.
pub fn in_deck_order(deck: &str, rows: Vec<MeasureResult>, backend: Dialect) -> Vec<MeasureResult> {
    let given = rows.len();
    let silent = matches!(backend, Dialect::Ngspice | Dialect::Espice);
    let mut rest: Vec<Option<MeasureResult>> = rows.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(given);
    let mut seen: Vec<&str> = Vec::new();
    for (analysis, name) in cards(deck) {
        if seen.iter().any(|s| s.eq_ignore_ascii_case(name)) {
            continue;
        }
        seen.push(name);
        let before = out.len();
        for slot in &mut rest {
            if let Some(mut row) = slot.take_if(|r| r.name.eq_ignore_ascii_case(name)) {
                row.analysis = analysis.to_ascii_lowercase();
                out.push(row);
            }
        }
        if out.len() == before && silent {
            // ngspice prints names in lowercase.
            let mut row = MeasureResult::failed(&name.to_ascii_lowercase());
            row.analysis = analysis.to_ascii_lowercase();
            out.push(row);
        }
    }
    out.extend(rest.into_iter().flatten());
    assert!(out.len() >= given, "no row is lost");
    assert!(
        out.iter()
            .all(|m| m.value.is_nan() == (m.status == MeasureStatus::Failed)),
        "a failed measure, and only one, has no value"
    );
    out
}

/// The rows of `rows` that hold at step `k` of a stepped run (ZERO D42:
/// its plot `k`): step `k`'s and those of no step. A run that does not
/// step has only rows of no step: every plot gets them all.
pub fn at_step(rows: &[MeasureResult], k: usize) -> Vec<MeasureResult> {
    rows.iter()
        .filter(|m| m.step.is_none_or(|s| s as usize == k))
        .cloned()
        .collect()
}

/// The `.four` tables in `text` (ngspice's stdout or LTspice's log), one
/// per output in the order printed; empty when `text` holds none. A
/// table's rows are the lines after its header that count on from its last
/// harmonic, so a later table of numbers is not read into it.
pub fn parse_fourier(text: &str) -> Vec<Fourier> {
    let mut tables: Vec<Fourier> = Vec::new();
    // ngspice's row is magnitude, phase, norm. mag, norm. phase; LTspice's
    // is component, normalized component, phase, normalized phase.
    let mut ltspice = false;
    for line in text.lines().map(str::trim) {
        let ngspice_header = line.strip_prefix("Fourier analysis for ");
        let ltspice_header = line.strip_prefix("Fourier components of ");
        if let Some(output) = ngspice_header.or(ltspice_header) {
            ltspice = ltspice_header.is_some();
            tables.push(Fourier {
                output: output.trim_end_matches(':').to_string(),
                thd: f64::NAN,
                ..Fourier::default()
            });
            continue;
        }
        let Some(table) = tables.last_mut() else {
            continue;
        };
        if let Some(thd) = distortion(line) {
            table.thd = thd;
            continue;
        }
        let rows = table.frequency.len();
        let dc = line.strip_prefix("DC component:");
        let row = match dc.and_then(|v| v.trim().parse::<f64>().ok()) {
            Some(dc) if rows == 0 => Some([0.0, dc, 0.0, 0.0, 0.0]),
            _ => harmonic_row(line, rows),
        };
        let Some([frequency, magnitude, b, c, norm_phase]) = row else {
            continue;
        };
        let (phase, norm_magnitude) = if ltspice { (c, b) } else { (b, c) };
        table.frequency.push(frequency);
        table.magnitude.push(magnitude);
        table.phase.push(phase);
        table.norm_magnitude.push(norm_magnitude);
        table.norm_phase.push(norm_phase);
    }
    for t in &tables {
        assert_eq!(t.magnitude.len(), t.frequency.len(), "{}", t.output);
        assert_eq!(t.norm_phase.len(), t.frequency.len(), "{}", t.output);
    }
    tables
}

// ---- private ----

/// `line` as harmonic `n`'s row less its number: the five values after it.
/// `None` when `line` is not six numbers, the first of them `n`. A `°`
/// after a value (LTspice's phases) is no part of it.
fn harmonic_row(line: &str, n: usize) -> Option<[f64; 5]> {
    let mut fields = line
        .split_whitespace()
        .map(|f| f.trim_end_matches('°').parse::<f64>().ok());
    if fields.next()?? != n as f64 {
        return None;
    }
    let mut row = [0.0; 5];
    for value in &mut row {
        *value = fields.next()??;
    }
    fields.next().is_none().then_some(row)
}

/// The total harmonic distortion `line` states, in percent: ngspice's
/// `THD: 9.99968 %`, LTspice's `Total Harmonic Distortion:   9.999731%`.
fn distortion(line: &str) -> Option<f64> {
    let (_, rest) = line
        .split_once("THD:")
        .or_else(|| line.split_once("Total Harmonic Distortion:"))?;
    rest.split_once('%')?.0.trim().parse().ok()
}

/// Parse NGSpice batch-mode stdout for .meas results.
///
/// Format: `name = value` (possibly with leading whitespace)
/// Lines containing "=" that look like measure results.
/// A `param` measure with no value is `name = failed`, a row too.
fn parse_ngspice(text: &str) -> Vec<MeasureResult> {
    // ngspice groups real results under a "Measurements for <Analysis>" header:
    //
    //     Measurements for Transient Analysis
    //
    //     tplh   =  9.975241e-12 targ=  1.059975e-09 trig=  1.050000e-09
    //
    // Anchoring to that block matters because ngspice's resource footer ends
    // with `Stack = 0 bytes.`, which a bare `name = value` scan reports as a
    // measurement named "Stack". Trailing text after the value is legitimate
    // (targ/trig/at/from/to), so the value cannot be required to stand alone.
    let mut results: Vec<MeasureResult> = Vec::new();
    let mut in_block = false;
    let mut saw_block = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.contains("Measurements for") {
            in_block = true;
            saw_block = true;
            continue;
        }
        if !in_block {
            continue;
        }
        if trimmed.is_empty() {
            continue;
        }
        match measure_line(trimmed) {
            Some(result) => push_measure(&mut results, result),
            // First non-measure line closes the block.
            None => in_block = false,
        }
    }

    if saw_block {
        return results;
    }

    // No header (non-batch output, or a caller passing a bare fragment):
    // fall back to scanning every line.
    for line in text.lines() {
        if let Some(result) = measure_line(line.trim()) {
            push_measure(&mut results, result);
        }
    }
    results
}

/// Parse a single `name = value [extra...]` measure line, or ngspice's
/// `name = failed`.
fn measure_line(trimmed: &str) -> Option<MeasureResult> {
    let (name_part, value_part) = trimmed.split_once('=')?;
    let name = name_part.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    let value_str = value_part.trim();
    if value_str.starts_with("failed") {
        return Some(MeasureResult::failed(name));
    }
    let value = value_str.split_whitespace().next()?.parse::<f64>().ok()?;
    Some(MeasureResult::ok(name, value))
}

/// Later values win: a deck that reports a measure twice should not yield
/// duplicate entries.
fn push_measure(results: &mut Vec<MeasureResult>, result: MeasureResult) {
    if let Some(existing) = results.iter_mut().find(|r| r.name == result.name) {
        *existing = result;
    } else {
        results.push(result);
    }
}

/// LTspice's log: a line per measure, or with `.step` a table per measure
/// with a row per step (the formats LTspice 26.1.1 writes, in the header).
fn parse_ltspice(text: &str) -> Vec<MeasureResult> {
    let mut results = Vec::new();
    // The measure whose step table the lines being read belong to.
    let mut table: Option<&str> = None;

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // `Measurement "tnever" FAIL'ed`: no value, at any step.
        if let Some(name) = trimmed
            .strip_prefix("Measurement \"")
            .and_then(|r| r.strip_suffix("\" FAIL'ed"))
        {
            results.push(MeasureResult::failed(name));
            table = None;
            continue;
        }
        // `Measurement: vmax`, a `step MAX(V(out)) FROM TO` header, then
        // a row per step, `<step> <value> ...` or `<step> failed`, the
        // steps counted from 1.
        if let Some(name) = trimmed.strip_prefix("Measurement:") {
            table = Some(name.trim());
            continue;
        }
        if let Some(name) = table {
            let mut words = trimmed.split_whitespace();
            let first = words.next().unwrap_or("");
            if first == "step" {
                continue;
            }
            if let (Ok(k), Some(value)) = (first.parse::<u32>(), words.next())
                && k >= 1
            {
                let mut row = MeasureResult::ok(name, value.parse().unwrap_or(f64::NAN));
                row.step = Some(k - 1);
                results.push(row);
                continue;
            }
            // Any other line ends the table and is read on its own.
            table = None;
        }

        if LTSPICE_NOT_MEASURES.iter().any(|p| trimmed.starts_with(p)) {
            continue;
        }

        // `trf=6e-07 FROM 2e-07 TO 8e-07`: a TRIG/TARG measure, no colon.
        if let Some((name, rest)) = trimmed.split_once('=')
            && !name.is_empty()
            && !name.contains([' ', ':'])
            && rest.contains(" FROM ")
        {
            let value = rest.split_whitespace().next().unwrap_or("");
            results.push(MeasureResult::ok(name, value.parse().unwrap_or(f64::NAN)));
            continue;
        }

        // Pattern: "name: value" or "name: KEY=value"
        if let Some((name_part, rest)) = trimmed.split_once(':') {
            let name = name_part.trim();

            // Skip if name has spaces (not a simple measure name) or is empty
            if name.is_empty() || name.contains(' ') {
                continue;
            }

            let rest = rest.trim();

            // Try direct numeric value: "name: 1.23e-09"
            let first_token = rest.split_whitespace().next().unwrap_or("");
            if let Ok(value) = first_token.parse::<f64>() {
                results.push(MeasureResult::ok(name, value));
                continue;
            }

            // `thalf: V(out)=0.3 AT 6e-07`: a WHEN measure is the point
            // after `AT` (a FIND ... AT writes `at`, and is the value
            // before it).
            if let Some((_, at)) = rest.rsplit_once(" AT ")
                && let Ok(value) = at.trim().parse::<f64>()
            {
                results.push(MeasureResult::ok(name, value));
                continue;
            }

            // Try "KEY=value" format: "name: AVG=1.23e-09" or "name: FROM=... TO=... AVG=..."
            for part in rest.split_whitespace() {
                if let Some((_key, val_str)) = part.split_once('=')
                    && let Ok(value) = val_str.parse::<f64>()
                {
                    results.push(MeasureResult::ok(name, value));
                    break; // Take first parseable value
                }
            }
        }
    }

    results
}

/// The `.meas` cards of `deck` in deck order, each its analysis word (empty
/// when the card has none) and its measure's name, as written.
fn cards(deck: &str) -> impl Iterator<Item = (&str, &str)> {
    deck.lines().filter_map(|line| {
        let mut words = line.split_whitespace();
        // `.meas`, `.measure` and what lies between, any case (as
        // `backend::ngspice` tells a measure card).
        let head = words.next()?.get(..5)?;
        if !head.eq_ignore_ascii_case(".meas") {
            return None;
        }
        let first = words.next()?;
        match MEAS_ANALYSES.iter().any(|a| first.eq_ignore_ascii_case(a)) {
            true => Some((first, words.next()?)),
            false => Some(("", first)),
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The deck ngspice printed [`NG_STDOUT`] for: 1k into 1 uF, a 0 to 1 V
    /// step. `vmid` is on a dc analysis the deck does not run.
    const NG_DECK: &str = "\
* rc\n\
R1 in out 1k\n\
C1 out 0 1u\n\
Vin in 0 0 AC 1 PULSE(0 1 0 0.000000001 0.000000001 1 2)\n\
.meas tran vend find v(out) at=5m\n\
.meas tran tnever when v(out)=2\n\
.meas tran p1 param='vend*2'\n\
.meas tran p2 param='tnever*2'\n\
.meas tran p3 param='vend*3'\n\
.meas ac gain1k find vdb(out) at=1k\n\
.meas ac fnever when vdb(out)=10\n\
.meas dc vmid find v(out) at=0.5\n\
.control\n\
tran 10u 5m 0 10u\n\
ac dec 10 1 100k\n\
.endc\n\
.end\n\
";

    /// ngspice-45's stdout for [`NG_DECK`] (`ngspice -b -n`, 2026-11-11), from
    /// its first measure block on, less the memory lines. Its stderr names
    /// `tnever` and `fnever` ("Error: measure  tnever  when(WHEN) : out of
    /// interval", then the card and "failed!").
    const NG_STDOUT: &str = "\
\x20 Measurements for Transient Analysis\n\
\n\
vend                =  9.932623e-01\n\
p1                  =  1.98652e+00\n\
p2                  =   failed\n\
p3                  =  2.97979e+00\n\
\n\
Doing analysis at TEMP = 27.000000 and TNOM = 27.000000\n\
\n\
Using SPARSE 1.3 as Direct Linear Solver\n\
\n\
No. of Data Rows : 51\n\
\n\
\x20 Measurements for Transient Analysis\n\
\n\
gain1k              =  -1.607224e+01\n\
\n\
Doing analysis at TEMP = 27.000000 and TNOM = 27.000000\n\
\n\
Using SPARSE 1.3 as Direct Linear Solver\n\
\n\
\x20 Measurements for Transient Analysis\n\
\n\
gain1k              =  -1.607224e+01\n\
\n\
\n\
Total analysis time (seconds) = 0.00147703\n\
\n\
Total elapsed time (seconds) = 0.006 \n\
\n\
Shared ngspice pages =   10.895 MB.\n\
Text (code) pages =    6.535 MB.\n\
Stack = 0 bytes.\n\
Library pages =    3.137 MB.\n\
\n\
";

    /// A 0 to 1 V pulse (1 us edges) over R1 = {r}, stepped 1k 2k 3k, into
    /// R2 = 1k: V(out) peaks at 1k/(r + 1k) = 0.5, 1/3, 0.25.
    const LT_DECK: &str = "\
* p8-102 stepped divider, tran, measures\n\
V1 in 0 PULSE(0 1 0 1u 1u 1u 4u)\n\
R1 in out {r}\n\
R2 out 0 1k\n\
.param r=1k\n\
.step param r list 1k 2k 3k\n\
.tran 1u 3u\n\
.meas tran vmax MAX V(out)\n\
.meas tran vat FIND V(out) AT=1.5u\n\
.meas tran tnever WHEN V(out)=2\n\
.meas tran thalf WHEN V(out)=0.3\n\
.end\n\
";

    /// LTspice 26.1.1's log (under Wine, 2026-11-11) for [`LT_DECK`], from its
    /// `Options:` line on (the measure lines end in `\r`).
    const LT_STEPPED: &str = "\
Options:plotwinsize=0 numdgt=15\n\
\n\
.step r=1000\n\
solver = Normal\n\
Maximum thread count: 32\n\
tnom = 27\n\
temp = 27\n\
method = trap\n\
.OP point found by inspection.\n\
.OP point found by inspection.\n\
\n\
.step r=2000\n\
.OP point found by inspection.\n\
.OP point found by inspection.\n\
\n\
.step r=3000\n\
.OP point found by inspection.\n\
.OP point found by inspection.\n\
Total elapsed time: 0.085 seconds.\n\
Total iterations: 2088\n\
\n\
Files loaded:\n\
Z:\\tmp\\p8-102\\step.cir\n\
\n\
\r\n\
Measurement: vmax\r\n\
\x20 step\tMAX(V(out))\tFROM\tTO\r\n\
\x20    1\t0.5\t0\t3e-06\r\n\
\x20    2\t0.333333333333\t0\t3e-06\r\n\
\x20    3\t0.25\t0\t3e-06\r\n\
\r\n\
Measurement: vat\r\n\
\x20 step\tV(out)\tat\r\n\
\x20    1\t0.5\t1.5e-06\r\n\
\x20    2\t0.333333333333\t1.5e-06\r\n\
\x20    3\t0.25\t1.5e-06\r\n\
Measurement \"tnever\" FAIL'ed\r\n\
\r\n\
Measurement: thalf\r\n\
\x20 step\tV(out)=0.3\r\n\
\x20    1\t6e-07\r\n\
\x20    2\t9e-07\r\n\
\x20    3\tfailed\r\n\
";

    /// LTspice 26.1.1's logs for the same divider with R1 = 1k and no
    /// `.step`: one run's log from its `Options:` line on, then the measure
    /// lines of a second run (`.meas tran thalf WHEN V(out)=0.3`, `trf TRIG
    /// V(out) VAL=0.1 RISE=1 TARG V(out) VAL=0.4 RISE=1`, `twice PARAM
    /// thalf*2`, `vmax MAX V(out)`) and of a third (`vfw FIND V(in) WHEN
    /// V(out)=0.3`, `slope DERIV V(out) AT=0.5u`, `area INTEG V(out)`, `pp
    /// PP V(out)`, `tcross WHEN V(out)=0.3 FALL=1`).
    const LT_ONE: &str = "\
Options:plotwinsize=0 numdgt=15\n\
solver = Normal\n\
Maximum thread count: 32\n\
tnom = 27\n\
temp = 27\n\
method = trap\n\
.OP point found by inspection.\n\
.OP point found by inspection.\n\
Total elapsed time: 0.014 seconds.\n\
Total iterations: 2088\n\
\n\
Files loaded:\n\
Z:\\tmp\\p8-102\\one.cir\n\
\n\
vmax: MAX(V(out))=0.5 FROM 0 TO 3e-06\r\n\
vat: V(out)=0.5 at 1.5e-06\r\n\
Measurement \"tnever\" FAIL'ed\r\n\
avg: AVG(V(out))=0.333333333333 FROM 0 TO 3e-06\r\n\
thalf: V(out)=0.3 AT 6e-07\r\n\
trf=6e-07 FROM 2e-07 TO 8e-07\r\n\
twice: thalf*2=1.2e-06\r\n\
vmax: MAX(V(out))=0.5 FROM 0 TO 3e-06\r\n\
vfw: V(in)=0.6 at 6e-07\r\n\
slope: V(out)=500000 at 5e-07\r\n\
area: INTEG(V(out))=1e-06 FROM 0 TO 3e-06\r\n\
pp: PP(V(out))=0.5 FROM 0 TO 3e-06\r\n\
tcross: V(out)=0.3 AT 2.4e-06\r\n\
";

    /// A row as the tests compare it: name, analysis, step, and the value
    /// (`None` when the measure failed).
    type Row<'a> = (&'a str, &'a str, Option<u32>, Option<f64>);

    fn fields(rows: &[MeasureResult]) -> Vec<Row<'_>> {
        rows.iter()
            .map(|m| {
                let value = (m.status == MeasureStatus::Ok).then_some(m.value);
                (m.name.as_str(), m.analysis.as_str(), m.step, value)
            })
            .collect()
    }

    #[test]
    fn p8_102_ngspice_printout_field_by_field() {
        // Oracle: ngspice-45's own printout of NG_DECK (NG_STDOUT, and its
        // stderr in research/impl/evidence/P8-102). vend, p1, p3 and gain1k
        // have the values it printed; p2 is its `failed`; tnever and
        // fnever are the two its stderr names; vmid is on an analysis that
        // did not run. Its header calls the ac block "Transient Analysis"
        // and it prints that block twice: the analysis is the card's.
        let printed = parse_measures(NG_STDOUT, Some(Dialect::Ngspice));
        let names: Vec<&str> = printed.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["vend", "p1", "p2", "p3", "gain1k"]);
        let rows = in_deck_order(NG_DECK, printed, Dialect::Ngspice);
        assert_eq!(
            fields(&rows),
            [
                ("vend", "tran", None, Some(0.9932623)),
                ("tnever", "tran", None, None),
                ("p1", "tran", None, Some(1.98652)),
                ("p2", "tran", None, None),
                ("p3", "tran", None, Some(2.97979)),
                ("gain1k", "ac", None, Some(-16.07224)),
                ("fnever", "ac", None, None),
                ("vmid", "dc", None, None),
            ]
        );
        // Every plot of a run that does not step carries every row.
        assert_eq!(at_step(&rows, 1).len(), rows.len());
    }

    #[test]
    fn p8_102_ltspice_stepped_log_one_row_per_step() {
        // Oracle: LTspice 26.1.1's own log of LT_DECK, and the closed form
        // (the divider's peak 1k/(r + 1k); V(out) = 0.3 where the ramp is
        // at 0.3 (r + 1k)/1k V: 0.6 us, 0.9 us, never for r = 3k).
        let rows = in_deck_order(
            LT_DECK,
            parse_measures(LT_STEPPED, Some(Dialect::Ltspice)),
            Dialect::Ltspice,
        );
        assert_eq!(
            fields(&rows),
            [
                ("vmax", "tran", Some(0), Some(0.5)),
                ("vmax", "tran", Some(1), Some(0.333333333333)),
                ("vmax", "tran", Some(2), Some(0.25)),
                ("vat", "tran", Some(0), Some(0.5)),
                ("vat", "tran", Some(1), Some(0.333333333333)),
                ("vat", "tran", Some(2), Some(0.25)),
                ("tnever", "tran", None, None),
                ("thalf", "tran", Some(0), Some(6e-7)),
                ("thalf", "tran", Some(1), Some(9e-7)),
                ("thalf", "tran", Some(2), None),
            ]
        );
        // The plot of step 2 (r = 3k): one row a measure.
        assert_eq!(
            fields(&at_step(&rows, 2)),
            [
                ("vmax", "tran", Some(2), Some(0.25)),
                ("vat", "tran", Some(2), Some(0.25)),
                ("tnever", "tran", None, None),
                ("thalf", "tran", Some(2), None),
            ]
        );
    }

    #[test]
    fn p8_102_ltspice_log_lines_of_each_measure_kind() {
        // Oracle: LTspice 26.1.1's own log lines (LT_ONE) and the closed
        // forms for R1 = R2 = 1k: the peak 0.5; the average of the 1 us
        // up, 1 us flat, 1 us down pulse over 3 us, 1/3; V(out) = 0.3 at
        // 0.6 us rising and 2.4 us falling (a WHEN is its time, not its
        // level); 0.1 to 0.4 V takes 0.6 us (a TRIG/TARG line has no
        // colon); V(in) = 0.6 where V(out) = 0.3 (a FIND ... WHEN is its
        // value: LTspice writes `at`, not `AT`); the edge's slope 0.5 V/us;
        // the pulse's area 0.5 V x 2 us.
        let rows = parse_measures(LT_ONE, Some(Dialect::Ltspice));
        assert_eq!(
            fields(&rows),
            [
                ("vmax", "", None, Some(0.5)),
                ("vat", "", None, Some(0.5)),
                ("tnever", "", None, None),
                ("avg", "", None, Some(0.333333333333)),
                ("thalf", "", None, Some(6e-7)),
                ("trf", "", None, Some(6e-7)),
                ("twice", "", None, Some(1.2e-6)),
                ("vmax", "", None, Some(0.5)),
                ("vfw", "", None, Some(0.6)),
                ("slope", "", None, Some(500000.0)),
                ("area", "", None, Some(1e-6)),
                ("pp", "", None, Some(0.5)),
                ("tcross", "", None, Some(2.4e-6)),
            ]
        );
    }

    #[test]
    fn test_parse_ngspice_measures() {
        let stdout = "\
Circuit: test circuit

Doing analysis at TEMP = 27.000000 and target TNOM = 27.000000

rise_time        =  2.345000e-09
fall_time        =  1.987000e-09
vout_dc          =  1.650000e+00
gain_failed      =  failed
";
        let results = parse_ngspice(stdout);
        assert_eq!(results.len(), 4);
        assert_eq!(results[3].name, "gain_failed");
        assert_eq!(results[3].status, MeasureStatus::Failed);
        assert!(results[3].value.is_nan());
        assert_eq!(results[0].name, "rise_time");
        assert!((results[0].value - 2.345e-9).abs() < 1e-20);
        assert_eq!(results[1].name, "fall_time");
        assert!((results[1].value - 1.987e-9).abs() < 1e-20);
        assert_eq!(results[2].name, "vout_dc");
        assert!((results[2].value - 1.65).abs() < 1e-10);
    }

    #[test]
    fn test_parse_ngspice_ignores_resource_footer() {
        // Verbatim ngspice -b output shape. `Stack = 0 bytes.` parses as a
        // `name = value` pair and was previously reported as a measurement.
        let stdout = "\
Circuit: * rc

  Measurements for AC Analysis

gain_100            =  -1.445070e+00
tplh                =  9.975241e-12 targ=  1.059975e-09 trig=  1.050000e-09

binary raw file \"/tmp/x.raw\"

Total analysis time (seconds) = 0.0632234
Stack = 0 bytes.
Library pages =    2.059 MB.
";
        let results = parse_ngspice(stdout);
        assert_eq!(results.len(), 2, "got {:?}", results);
        assert_eq!(results[0].name, "gain_100");
        assert!((results[0].value - -1.445070).abs() < 1e-9);
        // Trailing targ=/trig= text must not break the value parse.
        assert_eq!(results[1].name, "tplh");
        assert!((results[1].value - 9.975241e-12).abs() < 1e-20);
        assert!(!results.iter().any(|r| r.name == "Stack"));
    }

    #[test]
    fn test_parse_ngspice_repeated_measure_keeps_last() {
        let stdout = "  Measurements for Transient Analysis\n\ng = 1.0\ng = 2.0\n";
        let results = parse_ngspice(stdout);
        assert_eq!(results.len(), 1);
        assert!((results[0].value - 2.0).abs() < 1e-12);
    }

    #[test]
    fn test_parse_ngspice_empty() {
        let stdout = "Circuit: test\nDoing analysis at TEMP = 27\n";
        let results = parse_ngspice(stdout);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_ltspice_simple() {
        let log = "\
Circuit: test
Date: Mon May 12 10:00:00 2026
rise_time: 2.345e-009
fall_time: 1.987e-009
";
        let results = parse_ltspice(log);
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].name, "rise_time");
        assert!((results[0].value - 2.345e-9).abs() < 1e-20);
        assert_eq!(results[1].name, "fall_time");
    }

    #[test]
    fn test_parse_ltspice_key_value() {
        let log = "\
Measurement: avg_vout
  avg_vout: AVG=1.650000e+00 FROM=0 TO=1e-06
";
        let results = parse_ltspice(log);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "avg_vout");
        assert!((results[0].value - 1.65).abs() < 1e-10);
    }

    #[test]
    fn an_ltspice_options_line_is_no_measure() {
        // LTspice 26.1.1 under Wine wrote this log for `* meas / V1 in 0 2 /
        // R1 in out 1k / R2 out 0 1k / .options plotwinsize=0 numdgt=15 /
        // .op / .meas op vout FIND V(out)` (2026-11-08): one measure, vout =
        // 1 V (the divider); its `Options:` echo is not a measure `Options`.
        let log = "\
LTspice 26.1.1 for Windows
Circuit: Z:\\tmp\\zp1-008\\m.cir
Start Time: Sun Nov  8 04:52:36 2026
Options:plotwinsize=0 numdgt=15
solver = Normal
tnom = 27
Direct Newton iteration succeeded in finding operating point.
Total elapsed time: 0.016 seconds.

vout: V(out)=1 at 2
";
        let results = parse_ltspice(log);
        assert_eq!(results.len(), 1, "{results:?}");
        assert_eq!(results[0].name, "vout");
        assert_eq!(results[0].value, 1.0);
    }

    #[test]
    fn test_parse_ltspice_empty() {
        let log = "Circuit: test\nTotal elapsed time: 0.5s\n";
        let results = parse_ltspice(log);
        assert!(results.is_empty());
    }

    #[test]
    fn test_parse_measures_auto_detect() {
        // NGSpice format with unknown backend falls back to trying all
        let stdout = "rise_time        =  2.345000e-09\n";
        let results = parse_measures(stdout, None);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "rise_time");
    }

    #[test]
    fn test_parse_measures_dispatches_correctly() {
        let ngspice_out =
            "  Measurements for Transient Analysis\n\nrise_time        =  2.345000e-09\n";
        let ltspice_out = "rise_time: 2.345e-009\n";

        let r1 = parse_measures(ngspice_out, Some(Dialect::Ngspice));
        let r3 = parse_measures(ltspice_out, Some(Dialect::Ltspice));

        assert_eq!(r1.len(), 1);
        assert_eq!(r3.len(), 1);
        assert_eq!(r1[0].name, "rise_time");
        assert_eq!(r3[0].name, "rise_time");
    }

    #[test]
    fn test_parse_ngspice_scientific_notation() {
        let stdout = "bw =  5.67890e+06\n";
        let results = parse_ngspice(stdout);
        assert_eq!(results.len(), 1);
        assert!((results[0].value - 5.6789e6).abs() < 1.0);
    }

    #[test]
    fn p3_049_ngspice_four_tables_are_read() {
        // ngspice-45's stdout for `fourier 1k v(in) v(mid)`: 1 V at 1 kHz in
        // series with 0.1 V at 3 kHz (research/impl/evidence/P3-049/ng.log;
        // the rows past harmonic 3, and past 1 of v(mid), left out).
        let stdout = "\
No. of Data Rows : 10008
Fourier analysis for v(in):
  No. Harmonics: 10, THD: 9.99968 %, Gridsize: 200, Interpolation Degree: 1

Harmonic Frequency   Magnitude   Phase       Norm. Mag   Norm. Phase
-------- ---------   ---------   -----       ---------   -----------
 0       0           2.77289e-14 0           0           0
 1       1000        0.999996    2.10116e-07 1           0
 2       2000        4.27847e-14 -87.107     4.27848e-14 -87.107
 3       3000        0.0999964   5.6732e-06  0.0999968   5.46309e-06

Fourier analysis for v(mid):
  No. Harmonics: 10, THD: 2.78969e+15 %, Gridsize: 200, Interpolation Degree: 1

Harmonic Frequency   Magnitude   Phase       Norm. Mag   Norm. Phase
-------- ---------   ---------   -----       ---------   -----------
 0       0           -4.6296e-16 0           0           0
 1       1000        3.58449e-15 -157.26     1           0
";
        let tables = parse_fourier(stdout);
        assert_eq!(tables.len(), 2);
        let t = &tables[0];
        assert_eq!((t.output.as_str(), t.thd), ("v(in)", 9.99968));
        assert_eq!(t.frequency, [0.0, 1000.0, 2000.0, 3000.0]);
        assert_eq!(t.magnitude, [2.77289e-14, 0.999996, 4.27847e-14, 0.0999964]);
        assert_eq!(t.phase, [0.0, 2.10116e-07, -87.107, 5.6732e-06]);
        assert_eq!(t.norm_magnitude, [0.0, 1.0, 4.27848e-14, 0.0999968]);
        assert_eq!(t.norm_phase, [0.0, 0.0, -87.107, 5.46309e-06]);
        assert_eq!(
            (tables[1].output.as_str(), tables[1].thd),
            ("v(mid)", 2.78969e15)
        );
        assert_eq!(tables[1].magnitude, [-4.6296e-16, 3.58449e-15]);
    }

    #[test]
    fn p3_049_ltspice_four_tables_are_read() {
        // LTspice 26.1.1's log for `.four 1k v(in)` on the same circuit
        // (research/impl/evidence/P3-049/ltspice-deck.log; harmonics 4 to 9
        // left out): the DC component on its own line, the normalized
        // component before the phase, a degree sign after each phase.
        let log = "Total elapsed time: 0.062 seconds.\n\nFourier components of v(in)\n\
            N-Period=1\nDC component:-4.27431e-12\n\n\
            Harmonic\tFrequency\t Fourier \tNormalized\t Phase  \tNormalized\n\
            Number \t  [Hz]   \tComponent\t Component\t[degree]\tPhase [deg]\n\
            1   \t 1.000e+03\t 1.000e+00\t 1.000e+00\t   90.00°\t    0.00°\n\
            2   \t 2.000e+03\t 4.398e-10\t 4.398e-10\t   90.98°\t    0.98°\n\
            3   \t 3.000e+03\t 1.000e-01\t 1.000e-01\t   90.00°\t    0.00°\n\
            Partial Harmonic Distortion: 9.999732%\n\
            Total Harmonic Distortion:   9.999731%\n";
        let tables = parse_fourier(log);
        assert_eq!(tables.len(), 1);
        let t = &tables[0];
        assert_eq!((t.output.as_str(), t.thd), ("v(in)", 9.999731));
        assert_eq!(t.frequency, [0.0, 1e3, 2e3, 3e3]);
        assert_eq!(t.magnitude, [-4.27431e-12, 1.0, 4.398e-10, 0.1]);
        assert_eq!(t.phase, [0.0, 90.0, 90.98, 90.0]);
        assert_eq!(t.norm_magnitude, [0.0, 1.0, 4.398e-10, 0.1]);
        assert_eq!(t.norm_phase, [0.0, 0.0, 0.98, 0.0]);
    }

    #[test]
    fn p3_049_a_four_table_ends_where_its_harmonics_stop_counting() {
        // The last line is six numbers (a `print` row), not harmonic 2.
        let text = "Fourier analysis for v(a):\n 0 0 1 0 0 0\n 1 5 2 0 1 0\n\n 0 5 5 5 5 5\n";
        let tables = parse_fourier(text);
        assert_eq!(tables[0].frequency, [0.0, 5.0]);
        assert!(tables[0].thd.is_nan(), "no THD line: {}", tables[0].thd);
        // No header, no table.
        assert!(parse_fourier("vout = 1.0\n 0 0 1 0 0 0\n").is_empty());
    }
}
