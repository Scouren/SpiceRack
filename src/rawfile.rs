//! Reader and writer for nutmeg-format `.raw` files (ngspice, VACASK, LTspice, Spectre nutbin and nutascii):
//! `key: value` header lines, a `Variables:` block, then `Binary:` or `Values:` and the data.
//! [`parse_raw`] reads the first plot; [`parse_raw_all`] reads every plot
//! (ngspice writes one per analysis, not in card order); [`write_raw`]
//! writes plots as one ngspice raw file.
//!
//! Raw-file bytes in, [`RawData`] plots out, and back.
//!
//! Cites: none (the nutmeg raw format as ngspice, VACASK and LTspice write
//! it; ngspice-45's `load` of what [`write_raw`] writes is recorded in
//! `tests/fixtures/raw/README`).
//!
//! Contents:
//! - `pub enum RawFileError`, `pub enum RawFormat`
//! - `pub fn parse_raw(data: &[u8]) -> Result<RawData, RawFileError>`
//! - `pub fn parse_raw_all(data: &[u8]) -> Result<Vec<RawData>, RawFileError>`
//! - `pub fn write_raw(plots: &[RawData], format: RawFormat) -> Vec<u8>`

use crate::result::{RawData, VarInfo, VarType};
use num_complex::Complex64;
use std::fmt::Write as _;

#[derive(Debug, thiserror::Error)]
pub enum RawFileError {
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Unexpected format: {0}")]
    Format(String),
}

/// How [`write_raw`] lays out a plot's values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawFormat {
    /// `Binary:`, then each value a little-endian f64 (ngspice's default).
    Binary,
    /// `Values:`, then each value as text, the shortest that reads back to
    /// the same f64 (the layout of ngspice's `set filetype=ascii`).
    Ascii,
}

/// Parse a .raw file from bytes. Handles UTF-8 and LTspice's UTF-16-LE headers.
pub fn parse_raw(data: &[u8]) -> Result<RawData, RawFileError> {
    if utf16(data) {
        let first = parse_raw_all(data)?.into_iter().next();
        return Ok(first.expect("a raw file read holds at least one plot"));
    }
    parse_utf8(data, false).map(|(r, _, _)| r)
}

/// Every plot in the file, in file order. LTspice's UTF-16 files (with or
/// without a byte-order mark) hold one, or with `.step` one stepped plot,
/// read as one plot per step (ZERO D42): a stepped operating point's steps
/// carry their [`RawData::params`] (its column 0), any other's are the
/// log's (`backend::ltspice`).
pub fn parse_raw_all(data: &[u8]) -> Result<Vec<RawData>, RawFileError> {
    if utf16(data) {
        let (raw, _, stepped) = parse_utf8(&decode_utf16_le(data), true)?;
        return Ok(match stepped {
            true => split_steps(raw),
            false => vec![raw],
        });
    }
    let (first, mut used, _) = parse_utf8(data, false)?;
    let mut plots = vec![first];
    // Every later plot starts with `Title:`, or `Plotname:` (Xyce 7.10's
    // `-r` file, its second plot). Anything else is trailing junk: ngspice
    // with `.options interp` writes past the rows it declares.
    while [&b"Title:"[..], b"Plotname:"]
        .iter()
        .any(|h| data[used..].trim_ascii_start().starts_with(h))
    {
        let (raw, n, _) = parse_utf8(&data[used..], false)?;
        plots.push(raw);
        used += n;
    }
    Ok(plots)
}

/// `plots` as one ngspice raw file, plot after plot in the order given.
/// [`parse_raw_all`] reads it back with every title, plot name, variable
/// and value the same bit for bit (`Ascii`: a NaN reads back as a NaN, its
/// payload not kept), and ngspice's `load` reads it. A plot's points are
/// column 0's; a complex plot's real-valued column is written with a zero
/// imaginary part; a type [`VarType`] has no word for is `notype`. What a
/// raw file has no place for is not written: `params`, `stdout`,
/// `log_content`, `measures`, `backend_hint`. No `Date:` line: the same
/// plots give the same bytes.
///
/// # Panics
/// On a plot no raw file can hold: columns of different lengths,
/// `variables[i].index != i`, a variable name that is empty or holds
/// whitespace, a line break in the title or the plot name.
pub fn write_raw(plots: &[RawData], format: RawFormat) -> Vec<u8> {
    let mut out = Vec::new();
    for plot in plots {
        let before = out.len();
        write_plot(&mut out, plot, format);
        assert!(out.len() > before, "a plot is at least its header");
    }
    assert!(plots.is_empty() == out.is_empty());
    out
}

// ---- private ----

/// LTspice's UTF-16-LE header: a byte-order mark, or none (LTspice 26
/// writes none) and ASCII's zero high bytes.
fn utf16(data: &[u8]) -> bool {
    data.starts_with(&[0xFF, 0xFE])
        || (data.len() >= 10 && data[1] == 0 && data[3] == 0 && data[5] == 0)
}

/// LTspice header text is UTF-16-LE (one unit a character); binary data after
/// `Binary:` is copied through untouched.
fn decode_utf16_le(data: &[u8]) -> Vec<u8> {
    let body = data.strip_prefix(&[0xFF, 0xFE]).unwrap_or(data);
    let mut out = Vec::with_capacity(body.len() / 2);
    for (k, pair) in body.chunks_exact(2).enumerate() {
        // A header character outside ASCII (`Hz½` in a noise plot's name)
        // is one UTF-16 unit, written as its UTF-8 bytes.
        let c = char::from_u32(u32::from(u16::from_le_bytes([pair[0], pair[1]])));
        let mut buf = [0u8; 4];
        out.extend_from_slice(c.unwrap_or('?').encode_utf8(&mut buf).as_bytes());
        if pair[0] == b'\n' && out.trim_ascii_end().ends_with(b"Binary:") {
            out.extend_from_slice(&body[2 * k + 2..]);
            break;
        }
    }
    out
}

fn bad<E: std::fmt::Display>(what: &str) -> impl FnOnce(E) -> RawFileError + '_ {
    move |e| RawFileError::Parse(format!("Bad {what}: {e}"))
}

/// One plot from the start of `data`, the bytes it spans, and whether it
/// is stepped; `ltspice`: the header was LTspice's UTF-16 (its element
/// widths, [`parse_binary`]).
fn parse_utf8(data: &[u8], ltspice: bool) -> Result<(RawData, usize, bool), RawFileError> {
    let mut raw = RawData::default();
    let (mut num_vars, mut num_points) = (0usize, 0usize);
    let (mut in_variables, mut fast_access, mut double) = (false, false, !ltspice);
    let (mut stepped, mut padded) = (false, false);
    let mut pos = 0;

    let is_binary = loop {
        let nl = data[pos..]
            .iter()
            .position(|&b| b == b'\n')
            .ok_or_else(|| RawFileError::Format("Unexpected EOF in header".into()))?;
        let line = std::str::from_utf8(&data[pos..pos + nl])
            .map_err(bad("header text"))?
            .trim_end();
        pos += nl + 1;

        match line {
            "Binary:" => break true,
            "Values:" => break false,
            _ if line.starts_with("Variables:") => in_variables = true,
            _ if in_variables => {
                // "\tindex\tname\ttype [extra...]"
                let mut parts = line.split_whitespace();
                if let (Some(i), Some(name), Some(ty)) = (parts.next(), parts.next(), parts.next())
                {
                    raw.variables.push(VarInfo {
                        index: i.parse().map_err(bad("variable index"))?,
                        name: name.to_string(),
                        var_type: VarType::parse(ty),
                    });
                }
            }
            _ => {
                if let Some((key, value)) = line.split_once(':') {
                    let value = value.trim();
                    match key.trim() {
                        "Title" => raw.title = value.to_string(),
                        "Plotname" => raw.plot_name = value.to_string(),
                        "Flags" => {
                            let flags = value.to_lowercase();
                            raw.is_complex = flags.contains("complex");
                            fast_access = flags.contains("fastaccess");
                            double |= flags.contains("double");
                            stepped = flags.contains("stepped");
                            // ngspice's own flag (`padded`, `unpadded`),
                            // which `write_raw` always writes.
                            padded = flags.contains("padded");
                        }
                        "No. Variables" => num_vars = value.parse().map_err(bad("num_vars"))?,
                        "No. Points" => num_points = value.parse().map_err(bad("num_points"))?,
                        _ => {}
                    }
                }
            }
        }
    };

    // An empty plot (0 points: a sweep that failed; 0 variables: a circuit
    // with no nodes) is well-formed and reads as empty columns.
    if raw.variables.len() != num_vars {
        return Err(RawFileError::Format(format!(
            "Header declares {num_vars} variables but {} were listed",
            raw.variables.len()
        )));
    }
    if let Some(v) = raw.variables.iter().find(|v| v.index >= num_vars) {
        return Err(RawFileError::Format(format!(
            "Variable '{}' declares out-of-range index {} (num_vars = {num_vars})",
            v.name, v.index
        )));
    }

    let body = &data[pos..];
    let used = if is_binary {
        // Spectre's nutbin by its plot name, unless the flags are ngspice's.
        let big = !padded && big_endian(&raw.plot_name);
        parse_binary(
            body,
            &mut raw,
            num_vars,
            num_points,
            fast_access,
            double,
            big,
        )?
    } else {
        // The values end where the next plot's header begins; a plot with
        // no points has none, and the next header may start right here.
        let end = match num_points {
            0 => 0,
            _ => [&b"\nTitle:"[..], b"\nPlotname:"]
                .iter()
                .filter_map(|h| body.windows(h.len()).position(|w| w == *h))
                .min()
                .map_or(body.len(), |i| i + 1),
        };
        parse_ascii(&body[..end], &mut raw, num_vars, num_points)?;
        end
    };
    Ok((raw, pos + used, stepped))
}

/// A stepped plot as one plot per step. Column 0 a `param` (a stepped
/// operating point): one point a step, the step's value that point's.
/// Otherwise a step starts where column 0 comes back to its first value
/// (time 0, the first frequency or sweep value) after leaving it.
fn split_steps(raw: RawData) -> Vec<RawData> {
    let axis = raw.re(0).into_owned();
    if axis.is_empty() {
        return vec![raw];
    }
    let param = raw.variables[0].var_type == VarType::Param;
    let mut starts: Vec<usize> = (0..axis.len())
        .filter(|&i| param || i == 0 || (axis[i] == axis[0] && axis[i - 1] != axis[0]))
        .collect();
    starts.push(axis.len());
    starts
        .windows(2)
        .map(|w| RawData {
            title: raw.title.clone(),
            plot_name: raw.plot_name.clone(),
            variables: raw.variables.clone(),
            real_data: raw
                .real_data
                .iter()
                .map(|c| c[w[0]..w[1]].to_vec())
                .collect(),
            complex_data: raw
                .complex_data
                .iter()
                .map(|c| c[w[0]..w[1]].to_vec())
                .collect(),
            is_complex: raw.is_complex,
            params: match param {
                true => vec![(raw.variables[0].name.clone(), axis[w[0]])],
                false => Vec::new(),
            },
            ..RawData::default()
        })
        .collect()
}

/// Little-endian values (`big`: big-endian, Spectre's nutbin,
/// [`big_endian`]), point-major (`p*nv + v`) or, with FastAccess,
/// variable-major: f64 each, or, unless `double` (an LTspice raw without
/// its `double` flag), the scale (variable 0) f64 and the rest float32; a
/// complex plot's pairs are f64 always (settle-api row 15, measured).
/// One length check, then each column is gathered straight from the bytes.
fn parse_binary(
    body: &[u8],
    raw: &mut RawData,
    nv: usize,
    np: usize,
    fast_access: bool,
    double: bool,
    big: bool,
) -> Result<usize, RawFileError> {
    if !raw.is_complex && !double {
        return parse_f32(body, raw, nv, np);
    }
    let width = if raw.is_complex { 2 } else { 1 };
    let need = nv * np * width * 8;
    if body.len() < need {
        return Err(RawFileError::Format(format!(
            "Binary block is {} bytes, expected {need}",
            body.len()
        )));
    }
    let f = |j: usize| {
        let b: [u8; 8] = body[j * 8..j * 8 + 8].try_into().expect("8-byte slice");
        match big {
            true => f64::from_be_bytes(b),
            false => f64::from_le_bytes(b),
        }
    };
    let slot = |v: usize, p: usize| width * if fast_access { v * np + p } else { p * nv + v };
    if raw.is_complex {
        raw.complex_data = (0..nv)
            .map(|v| {
                (0..np)
                    .map(|p| Complex64::new(f(slot(v, p)), f(slot(v, p) + 1)))
                    .collect()
            })
            .collect();
    } else {
        raw.real_data = (0..nv)
            .map(|v| (0..np).map(|p| f(slot(v, p))).collect())
            .collect();
    }
    Ok(need)
}

/// Whether a binary plot is Spectre's: Spectre names a plot after its
/// analysis instance in `` `name' `` quotes (``DC Analysis `op1'``) and
/// writes nutbin big-endian IEEE f64 (settle-remote P1-040, Spectre 25.1);
/// ngspice, VACASK, Xyce and LTspice write little-endian, and so does
/// [`write_raw`], whose `padded` flag overrides the name (`parse_utf8`).
fn big_endian(plot_name: &str) -> bool {
    plot_name.contains('`')
}

/// [`parse_binary`]'s LTspice rows: the scale f64, the rest float32,
/// point-major (LTspice writes no FastAccess).
fn parse_f32(body: &[u8], raw: &mut RawData, nv: usize, np: usize) -> Result<usize, RawFileError> {
    let row = 8 + 4 * nv.saturating_sub(1);
    let need = row * np;
    if body.len() < need {
        return Err(RawFileError::Format(format!(
            "Binary block is {} bytes, expected {need}",
            body.len()
        )));
    }
    let at = |p: usize, v: usize| -> f64 {
        let o = p * row;
        match v {
            0 => f64::from_le_bytes(body[o..o + 8].try_into().expect("8 bytes")),
            _ => {
                let o = o + 8 + 4 * (v - 1);
                f32::from_le_bytes(body[o..o + 4].try_into().expect("4 bytes")).into()
            }
        }
    };
    raw.real_data = (0..nv)
        .map(|v| (0..np).map(|p| at(p, v)).collect())
        .collect();
    Ok(need)
}

/// Whitespace-separated: per point an index token, then one value per variable
/// (`re,im` for a complex plot).
fn parse_ascii(body: &[u8], raw: &mut RawData, nv: usize, np: usize) -> Result<(), RawFileError> {
    let text = std::str::from_utf8(body).map_err(bad("values text"))?;
    let mut tokens = text.split_ascii_whitespace();
    let mut next = || {
        tokens
            .next()
            .ok_or_else(|| RawFileError::Format("Values block ends early".into()))
    };
    let num = |s: &str| {
        s.parse::<f64>()
            .map_err(|e| RawFileError::Parse(format!("Bad value '{s}': {e}")))
    };

    if raw.is_complex {
        raw.complex_data = vec![Vec::with_capacity(np); nv];
    } else {
        raw.real_data = vec![Vec::with_capacity(np); nv];
    }
    for _ in 0..np {
        next()?; // point index
        for v in 0..nv {
            let tok = next()?;
            if raw.is_complex {
                let (re, im) = tok.split_once(',').unwrap_or((tok, "0"));
                raw.complex_data[v].push(Complex64::new(num(re)?, num(im)?));
            } else {
                raw.real_data[v].push(num(tok)?);
            }
        }
    }
    Ok(())
}

/// One plot appended to `out`: its header, then its values in `format`
/// ([`write_raw`], whose contract the assertions are).
fn write_plot(out: &mut Vec<u8>, plot: &RawData, format: RawFormat) {
    // A column's points: its complex values, else its real ones (the
    // order `RawData::re` reads a column in).
    let len = |v: usize| match plot.complex(v) {
        Some(c) => c.len(),
        None => plot.real_data.get(v).map_or(0, Vec::len),
    };
    let nv = plot.variables.len();
    let np = if nv == 0 { 0 } else { len(0) };
    assert!(
        !plot.title.contains('\n') && !plot.plot_name.contains('\n'),
        "a raw header line holds no line break"
    );
    assert!(
        (0..nv).all(|v| len(v) == np),
        "every column of `{}` holds {np} points",
        plot.plot_name
    );
    let kind = if plot.is_complex { "complex" } else { "real" };
    // `padded` is ngspice's flag for this layout (every column `np` long).
    // No Spectre nutbin carries it, so the reader takes a file with it as
    // little-endian whatever its plots are named (`parse_utf8`).
    let mut head = format!(
        "Title: {}\nPlotname: {}\nFlags: {kind} padded\nNo. Variables: {nv}\nNo. Points: {np}\nVariables:\n",
        plot.title, plot.plot_name
    );
    for (i, v) in plot.variables.iter().enumerate() {
        assert!(v.index == i, "variable `{}` is column {i}", v.name);
        assert!(
            !v.name.is_empty() && !v.name.contains(char::is_whitespace),
            "a variable's name is one word: `{}`",
            v.name
        );
        writeln!(head, "\t{i}\t{}\t{}", v.name, type_word(v.var_type))
            .expect("a String takes any write");
    }
    out.extend_from_slice(head.as_bytes());
    // ponytail: a column lookup per value, as `parse_binary`'s `slot`; one
    // kernel for the reader and the writer when P9-016 lands.
    let at = |v: usize, p: usize| match plot.complex(v) {
        Some(c) => c[p],
        None => Complex64::new(plot.real_data[v][p], 0.0),
    };
    match format {
        RawFormat::Binary => {
            out.extend_from_slice(b"Binary:\n");
            let start = out.len();
            for p in 0..np {
                for v in 0..nv {
                    let x = at(v, p);
                    out.extend_from_slice(&x.re.to_le_bytes());
                    if plot.is_complex {
                        out.extend_from_slice(&x.im.to_le_bytes());
                    }
                }
            }
            let width = if plot.is_complex { 16 } else { 8 };
            assert!(out.len() - start == nv * np * width);
        }
        RawFormat::Ascii => {
            // ` <point>`, a line per variable, a blank line after the
            // point; `{:e}` is the shortest text that reads back the same.
            let mut text = String::from("Values:\n");
            for p in 0..np {
                write!(text, " {p}").expect("a String takes any write");
                for v in 0..nv {
                    let x = at(v, p);
                    match plot.is_complex {
                        true => writeln!(text, "\t{:e},{:e}", x.re, x.im),
                        false => writeln!(text, "\t{:e}", x.re),
                    }
                    .expect("a String takes any write");
                }
                text.push('\n');
            }
            out.extend_from_slice(text.as_bytes());
        }
    }
}

/// The type word [`VarType::parse`] reads back as `t`: ngspice's `notype`
/// for [`VarType::Other`], whose word as read is not kept.
fn type_word(t: VarType) -> &'static str {
    match t {
        VarType::Voltage => "voltage",
        VarType::Current => "current",
        VarType::Time => "time",
        VarType::Frequency => "frequency",
        VarType::Param => "param",
        VarType::Other => "notype",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header(flags: &str, vars: &[&str], np: usize, marker: &str) -> Vec<u8> {
        let mut h = format!(
            "Title: t\nPlotname: p\nFlags: {flags}\nNo. Variables: {}\nNo. Points: {np}\nVariables:\n",
            vars.len()
        );
        for (i, v) in vars.iter().enumerate() {
            h += &format!("\t{i}\t{v}\tvoltage\n");
        }
        h += marker;
        h.into_bytes()
    }

    #[test]
    fn every_plot_in_order() {
        let mut ascii = header("real", &["v(a)"], 1, "Values:\n");
        ascii.extend_from_slice(b" 0\t1.5\n");
        ascii.extend(header("real", &["time", "v(a)"], 2, "Values:\n"));
        ascii.extend_from_slice(b" 0\t0\n\t1\n 1\t1e-9\n\t2\n");
        let plots = parse_raw_all(&ascii).unwrap();
        assert_eq!(plots.len(), 2);
        assert_eq!(plots[0].real_data, vec![vec![1.5]]);
        assert_eq!(plots[1].real_data, vec![vec![0.0, 1e-9], vec![1.0, 2.0]]);

        let mut bin = header("real", &["v(a)"], 1, "Binary:\n");
        bin.extend_from_slice(&3.0f64.to_le_bytes());
        bin.extend(header("real", &["v(a)", "v(b)"], 1, "Binary:\n"));
        bin.extend([4.0f64, 5.0].iter().flat_map(|v| v.to_le_bytes()));
        let plots = parse_raw_all(&bin).unwrap();
        assert_eq!(
            plots
                .iter()
                .map(|p| p.real_data.clone())
                .collect::<Vec<_>>(),
            vec![vec![vec![3.0]], vec![vec![4.0], vec![5.0]]]
        );
    }

    #[test]
    fn ascii_real_and_complex() {
        let mut real = header("real", &["v(out)", "v(in)"], 2, "Values:\n");
        real.extend_from_slice(b" 0\t3.3\n\t1.0\n\n 1\t3.4\n\t1.1\n");
        let r = parse_raw(&real).unwrap();
        assert_eq!(r.title, "t");
        assert_eq!(r.real_data, vec![vec![3.3, 3.4], vec![1.0, 1.1]]);

        let mut cplx = header("complex", &["frequency", "v(out)"], 1, "Values:\n");
        cplx.extend_from_slice(b" 0\t1e3,0\n\t3,4\n");
        let c = parse_raw(&cplx).unwrap();
        assert!(c.is_complex && c.real_data.is_empty());
        assert_eq!(c.complex(1).unwrap()[0], Complex64::new(3.0, 4.0));
        assert_eq!(c.re(0), vec![1e3]);
    }

    /// Same data written interleaved and FastAccess must parse identically.
    #[test]
    fn binary_layouts_agree() {
        let cols: [[f64; 3]; 2] = [[0.0, 1e-6, 2e-6], [1.1, 2.2, 3.3]];
        let mut normal = header("real", &["time", "v(out)"], 3, "Binary:\n");
        for p in 0..3 {
            for col in &cols {
                normal.extend_from_slice(&col[p].to_le_bytes());
            }
        }
        let mut fast = header("real fastaccess", &["time", "v(out)"], 3, "Binary:\n");
        for col in &cols {
            for x in col {
                fast.extend_from_slice(&x.to_le_bytes());
            }
        }
        let want: Vec<Vec<f64>> = cols.iter().map(|c| c.to_vec()).collect();
        assert_eq!(parse_raw(&normal).unwrap().real_data, want);
        assert_eq!(parse_raw(&fast).unwrap().real_data, want);

        normal.truncate(normal.len() - 1);
        assert!(parse_raw(&normal).is_err(), "short binary block must error");
    }

    #[test]
    fn empty_plots() {
        let mut buf = header("real", &["v(a)", "v(b)"], 0, "Binary:\n");
        buf.extend(header("real", &[], 1, "Binary:\n"));
        buf.extend(header("real", &["v(a)"], 1, "Binary:\n"));
        buf.extend_from_slice(&7.0f64.to_le_bytes());
        buf.extend_from_slice(&[0x9c, 0xff, 0x01]); // `.options interp` junk
        let plots = parse_raw_all(&buf).unwrap();
        assert_eq!(plots.len(), 3);
        assert_eq!(plots[0].real_data, vec![Vec::<f64>::new(); 2]);
        assert!(plots[1].real_data.is_empty());
        assert_eq!(plots[2].real_data, vec![vec![7.0]]);
    }

    #[test]
    fn p8_101_an_empty_ascii_plot_keeps_the_next_plots_header() {
        // A plot with no points has no values: the next plot's `Title:`
        // follows `Values:` directly (the layout `write_raw` gives it).
        // Oracle: the header the test wrote. Checks: 4.
        let mut buf = header("real", &["v(a)"], 0, "Values:\n");
        buf.extend(header("real", &["v(a)"], 1, "Values:\n"));
        buf.extend_from_slice(b" 0\t7\n\n");
        let plots = parse_raw_all(&buf).unwrap();
        assert_eq!(plots.len(), 2);
        assert_eq!(plots[0].real_data, vec![Vec::<f64>::new()]);
        assert_eq!(plots[1].title, "t");
        assert_eq!(plots[1].real_data, vec![vec![7.0]]);
    }

    #[test]
    fn binary_complex_interleaved() {
        let mut buf = header("complex", &["frequency", "v(out)"], 2, "Binary:\n");
        for x in [1e3, 0.0, 0.99, -0.01, 1e4, 0.0, 0.8, -0.1] {
            buf.extend_from_slice(&f64::to_le_bytes(x));
        }
        let r = parse_raw(&buf).unwrap();
        assert_eq!(r.re(0), vec![1e3, 1e4]);
        assert_eq!(r.complex(1).unwrap()[1], Complex64::new(0.8, -0.1));
    }

    #[test]
    fn utf16_header_with_binary_body() {
        let mut buf = vec![0xFF, 0xFE];
        for b in header("real", &["time"], 1, "Binary:\n") {
            buf.extend_from_slice(&[b, 0]);
        }
        buf.extend_from_slice(&2.5f64.to_le_bytes());
        assert_eq!(parse_raw(&buf).unwrap().real_data, vec![vec![2.5]]);
    }
}
