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
//! **LTspice** (log file):
//! ```text
//! Measurement: rise_time
//!   rise_time: AVG=2.345e-09
//! ```
//! Or single-line: `rise_time: 2.345e-009 ...`
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
//! - `pub fn parse_fourier(text: &str) -> Vec<Fourier>`

use crate::result::MeasureResult;
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
const LTSPICE_NOT_MEASURES: [&str; 6] = [
    "Circuit:",
    "Date:",
    "Total elapsed",
    ".step",
    "Measurement:",
    // The `.options` echo (`Options:plotwinsize=0 numdgt=15`).
    "Options:",
];

/// Parse measure results from `backend`'s output, in its format; with no
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
/// NGSpice also prints "failed" for measures that didn't trigger.
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

/// Parse a single `name = value [extra...]` measure line.
fn measure_line(trimmed: &str) -> Option<MeasureResult> {
    let (name_part, value_part) = trimmed.split_once('=')?;
    let name = name_part.trim();
    if name.is_empty() || name.contains(char::is_whitespace) {
        return None;
    }
    let value_str = value_part.trim();
    if value_str.starts_with("failed") {
        return None;
    }
    let value = value_str.split_whitespace().next()?.parse::<f64>().ok()?;
    Some(MeasureResult {
        name: name.to_string(),
        value,
    })
}

/// Later values win: a deck that reports a measure twice should not yield
/// duplicate entries.
fn push_measure(results: &mut Vec<MeasureResult>, result: MeasureResult) {
    if let Some(existing) = results.iter_mut().find(|r| r.name == result.name) {
        existing.value = result.value;
    } else {
        results.push(result);
    }
}

fn parse_ltspice(text: &str) -> Vec<MeasureResult> {
    let mut results = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if LTSPICE_NOT_MEASURES.iter().any(|p| trimmed.starts_with(p)) {
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
                results.push(MeasureResult {
                    name: name.to_string(),
                    value,
                });
                continue;
            }

            // Try "KEY=value" format: "name: AVG=1.23e-09" or "name: FROM=... TO=... AVG=..."
            for part in rest.split_whitespace() {
                if let Some((_key, val_str)) = part.split_once('=')
                    && let Ok(value) = val_str.parse::<f64>()
                {
                    results.push(MeasureResult {
                        name: name.to_string(),
                        value,
                    });
                    break; // Take first parseable value
                }
            }
        }
    }

    results
}

#[cfg(test)]
mod tests {
    use super::*;

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
        assert_eq!(results.len(), 3);
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
