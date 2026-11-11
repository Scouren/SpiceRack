//! HDL sources on their way to a simulator: Verilog-A models and digital
//! Verilog blocks, plus the external tools that compile them.
//!
//! `veriloga()` records the `.va` source in a subcircuit's load list; each
//! target's writer (`syntax::render`) decides what that becomes. ngspice and
//! VACASK load OSDI, so their writer compiles the source with OpenVAF
//! ([`compile_osdi`]); Spectre includes it with `ahdl_include`; ESPice
//! compiles it itself through VerA (`.hdl`), so no OpenVAF run happens on
//! that path.
//!
//! A digital Verilog block ([`lower`]) is either co-simulated (Icarus `.vvp`
//! under ngspice `d_cosim`) or synthesized by Yosys into standard cells.
//!
//! Cites: none (OpenVAF, Icarus and Yosys command lines; ngspice `d_cosim`).
//!
//! Contents:
//! - `pub struct VerilogBlock`, `pub enum VerilogMode`, `pub enum VerilogConnection`
//! - `pub fn is_source(path: &str) -> bool`
//! - `pub fn osdi_loads(loads: &[String]) -> Result<Vec<String>, String>` (without `backend`: the paths
//!   as given, or an error for a `.va` source)
//! - feature `backend`, from `mod run`: `pub fn resolve(source_or_path: &str) -> Result<PathBuf, String>`,
//!   `pub fn compile_osdi(va: &Path) -> Result<String, String>`,
//!   `pub fn osdi_loads(loads: &[String]) -> Result<Vec<String>, String>`, `pub enum Lowered`,
//!   `pub struct Cell`, `pub fn lower(vb: &VerilogBlock) -> Result<Lowered, String>`,
//!   `pub(super) fn module_name(text: &str) -> Option<&str>`,
//!   `pub(super) fn gate_instances(netlist: &str) -> Vec<(String, String, Vec<String>)>`

use std::collections::BTreeMap;

#[cfg(feature = "backend")]
pub use run::{Cell, Lowered, compile_osdi, lower, osdi_loads, resolve};

/// A digital Verilog block: its source (a path or inline text), whether it
/// is co-simulated or synthesized, the instance's name and its port nets.
#[derive(Debug, Clone, PartialEq)]
pub struct VerilogBlock {
    pub source: String,
    pub mode: VerilogMode,
    pub instance_name: String,
    pub connections: BTreeMap<String, VerilogConnection>,
    pub pdk: Option<String>,
    pub liberty: Option<String>,
    pub spice_models: Option<String>,
}

/// How a [`VerilogBlock`] reaches the simulator.
#[derive(Debug, Clone, PartialEq)]
pub enum VerilogMode {
    Simulate,
    Synthesize,
}

/// A Verilog port connection: single net or a bus (vector of nets)
#[derive(Debug, Clone, PartialEq)]
pub enum VerilogConnection {
    Single(String),
    Bus(Vec<String>),
}

/// True for a load-list entry that is Verilog-A source rather than a binary.
pub fn is_source(path: &str) -> bool {
    let p = std::path::Path::new(path);
    matches!(p.extension().and_then(|e| e.to_str()), Some(e) if e.eq_ignore_ascii_case("va") || e.eq_ignore_ascii_case("vams"))
}

/// The tool runs: OpenVAF, Icarus and Yosys, only with the `backend` feature.
#[cfg(feature = "backend")]
mod run {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::is_source;
    use super::{VerilogBlock, VerilogConnection, VerilogMode};

    /// The `.va` file for a path or inline source.
    pub fn resolve(source_or_path: &str) -> Result<PathBuf, String> {
        resolve_ext(source_or_path, "va")
    }

    /// An existing `.{ext}` file (made absolute, so a deck written to a temp dir
    /// still finds it), or inline source written to a temp file named by its
    /// hash, so the same text always maps to the same file. That file is
    /// rewritten only when it holds other text (its outputs stay newer),
    /// and through `private`: another process lowering the same block never
    /// reads it half written.
    fn resolve_ext(source_or_path: &str, ext: &str) -> Result<PathBuf, String> {
        let trimmed = source_or_path.trim();
        if trimmed.ends_with(&format!(".{ext}")) && !trimmed.contains('\n') {
            let p = Path::new(trimmed);
            if !p.exists() {
                return Err(format!(
                    "{} file not found: {}",
                    if ext == "va" { "Verilog-A" } else { "Verilog" },
                    trimmed
                ));
            }
            return p.canonicalize().map_err(|e| format!("{}: {}", trimmed, e));
        }
        let dir = std::env::temp_dir().join("spicerack_hdl");
        std::fs::create_dir_all(&dir).map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let hash = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            trimmed.hash(&mut h);
            h.finish()
        };
        let file = dir.join(format!("inline_{:016x}.{ext}", hash));
        if std::fs::read(&file).ok().as_deref() != Some(trimmed.as_bytes()) {
            let tmp = private(&file);
            std::fs::write(&tmp, trimmed)
                .and_then(|()| std::fs::rename(&tmp, &file))
                .map_err(|e| format!("Failed to write temp .{ext} file: {}", e))?;
        }
        Ok(file)
    }

    /// A path beside `p`, same extension, that no other writer uses (this
    /// process, this call): written there and renamed over `p`, a file is
    /// whole whenever another process opens it.
    fn private(p: &Path) -> PathBuf {
        static CALLS: AtomicU32 = AtomicU32::new(0);
        let n = CALLS.fetch_add(1, Ordering::Relaxed);
        let ext = p.extension().map(|e| e.to_string_lossy().into_owned());
        p.with_extension(format!(
            "{}-{n}.{}",
            std::process::id(),
            ext.unwrap_or_default()
        ))
    }

    /// Run `tool args(o)` to make `out` from `src`, skipped when `out` is
    /// newer: the tool writes `o` (`private`), renamed over `out` once it
    /// succeeds.
    fn run_tool(
        src: &Path,
        out: &Path,
        tool: &str,
        install: &str,
        args: impl Fn(&Path) -> Vec<OsString>,
    ) -> Result<String, String> {
        let mtime = |p: &Path| p.metadata().and_then(|m| m.modified());
        let done = out.to_string_lossy().into_owned();
        if let (Ok(s), Ok(o)) = (mtime(src), mtime(out))
            && o > s
        {
            return Ok(done);
        }
        let tmp = private(out);
        let output = Command::new(tool)
            .args(args(&tmp))
            .output()
            .map_err(|e| match e.kind() {
                std::io::ErrorKind::NotFound => format!("{tool} not found on $PATH. {install}"),
                _ => format!("Failed to run {tool}: {e}"),
            })?;
        if !output.status.success() {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!(
                "{tool} failed (exit {}):\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr),
                String::from_utf8_lossy(&output.stdout)
            ));
        }
        std::fs::rename(&tmp, out).map_err(|e| format!("{tool}: {}: {e}", out.display()))?;
        Ok(done)
    }

    /// Compile `va` to an `.osdi` beside it with OpenVAF.
    pub fn compile_osdi(va: &Path) -> Result<String, String> {
        let osdi = va.with_extension("osdi");
        let install =
            "Install OpenVAF to compile Verilog-A models.\nSee: https://openvaf.semimod.de";
        run_tool(va, &osdi, "openvaf", install, |o| {
            vec![va.into(), "-o".into(), o.into()]
        })
    }

    /// A load list with every Verilog-A source replaced by its compiled `.osdi`,
    /// for the backends that load OSDI.
    pub fn osdi_loads(loads: &[String]) -> Result<Vec<String>, String> {
        loads
            .iter()
            .map(|l| {
                if is_source(l) {
                    compile_osdi(Path::new(l))
                } else {
                    Ok(l.clone())
                }
            })
            .collect()
    }

    /// What a digital Verilog block becomes in a netlist.
    #[derive(Debug, PartialEq)]
    pub enum Lowered {
        /// ngspice XSPICE `d_cosim` running the compiled `vvp`; one `[net ...]`
        /// group per port.
        Cosim { vvp: String, ports: Vec<String> },
        /// Standard-cell subcircuit calls; `spice_models` holds the cell subckts.
        Gates {
            spice_models: String,
            cells: Vec<Cell>,
        },
    }

    /// One synthesized standard-cell instance.
    #[derive(Debug, PartialEq)]
    pub struct Cell {
        pub name: String,
        pub cell: String,
        pub nodes: Vec<String>,
    }

    fn nets(c: &VerilogConnection) -> &[String] {
        match c {
            VerilogConnection::Single(n) => std::slice::from_ref(n),
            VerilogConnection::Bus(v) => v,
        }
    }

    /// Compile (Icarus) or synthesize (Yosys) a Verilog block.
    pub fn lower(vb: &VerilogBlock) -> Result<Lowered, String> {
        let v = resolve_ext(&vb.source, "v")?;
        if matches!(vb.mode, VerilogMode::Simulate) {
            let vvp = v.with_extension("vvp");
            let install = "Install Icarus Verilog for co-simulation.\n\
                 Install: sudo apt install iverilog (Debian/Ubuntu) or brew install icarus-verilog (macOS)";
            let vvp = run_tool(&v, &vvp, "iverilog", install, |o| {
                vec!["-o".into(), o.into(), v.clone().into()]
            })?;
            let ports = vb
                .connections
                .values()
                .map(|c| format!("[{}]", nets(c).join(" ")))
                .collect();
            return Ok(Lowered::Cosim { vvp, ports });
        }

        let liberty = match (&vb.liberty, &vb.pdk) {
            (Some(l), _) => l.clone(),
            (None, Some(p)) => pdk_paths(p)?.0,
            (None, None) => return Err("mode='synthesize' requires either pdk or liberty".into()),
        };
        let spice_models = match (&vb.spice_models, &vb.pdk) {
            (Some(s), _) => s.clone(),
            (None, Some(p)) => pdk_paths(p)?.1,
            (None, None) => {
                return Err(
                    "mode='synthesize' requires either pdk or both liberty and spice_models".into(),
                );
            }
        };

        let text = std::fs::read_to_string(&v)
            .map_err(|e| format!("Failed to read {}: {}", v.display(), e))?;
        let top = module_name(&text)
            .ok_or("Could not extract module name from Verilog source. Ensure source contains `module <name>`.")?;
        let synth = v.with_extension("synth.v");
        let script = |o: &Path| {
            format!(
                "read_verilog {}; synth -top {top}; dfflibmap -liberty {liberty}; abc -liberty {liberty}; write_verilog {}",
                v.display(),
                o.display()
            )
        };
        let install = "Install Yosys for Verilog synthesis.\n\
             Install: sudo apt install yosys (Debian/Ubuntu) or brew install yosys (macOS)";
        let synth = run_tool(&v, &synth, "yosys", install, |o| {
            vec!["-p".into(), script(o).into()]
        })?;
        let netlist = std::fs::read_to_string(&synth)
            .map_err(|e| format!("Failed to read synthesized netlist: {}", e))?;

        // A synthesized net named like a user port lands on that port's first net;
        // every other net is prefixed with the instance name.
        let inst = &vb.instance_name;
        let cells = gate_instances(&netlist)
            .into_iter()
            .map(|(gate, cell, ports)| Cell {
                name: format!("{inst}_{gate}"),
                cell,
                nodes: ports
                    .into_iter()
                    .map(
                        |net| match vb.connections.get(&net).and_then(|c| nets(c).first()) {
                            Some(n) => n.clone(),
                            None => format!("{inst}_{net}"),
                        },
                    )
                    .collect(),
            })
            .collect();
        Ok(Lowered::Gates {
            spice_models,
            cells,
        })
    }

    /// The name after the first `module` that leads a line (blanks before
    /// it, at least one blank after it, maybe across lines): what the regex
    /// `(?m)^\s*module\s+(\w+)` captured.
    pub(super) fn module_name(text: &str) -> Option<&str> {
        line_starts(text).find_map(|s| {
            let rest = text[s..].trim_start().strip_prefix("module")?;
            let name = rest.trim_start();
            let n = word(name);
            (name.len() < rest.len() && n > 0).then(|| &name[..n])
        })
    }

    /// Cell instances of a Yosys `write_verilog` netlist as (instance, cell, nets
    /// in port order), e.g. `sky130_fd_sc_hd__and2_1 _0_ ( .A(\a ), .X(\out ) );`:
    /// what the regexes `(?m)^\s+(\w+)\s+(\w+)\s*\(([^;]+)\);` (an instance)
    /// and `\.\s*\w+\s*\(\s*\\?([^\s)]+)\s*\)` (a port) captured.
    pub(super) fn gate_instances(netlist: &str) -> Vec<(String, String, Vec<String>)> {
        let mut out = Vec::new();
        let mut from = 0;
        for s in line_starts(netlist) {
            if s < from {
                continue;
            }
            let Some((cell, inst, body, end)) = instance_at(netlist, s) else {
                continue;
            };
            from = end;
            if !matches!(
                cell,
                "wire" | "assign" | "input" | "output" | "module" | "endmodule" | "reg"
            ) {
                out.push((inst.to_string(), cell.to_string(), ports(body)));
            }
        }
        out
    }

    /// Every line's first byte offset in `text`.
    fn line_starts(text: &str) -> impl Iterator<Item = usize> + '_ {
        std::iter::once(0).chain(text.match_indices('\n').map(|(i, _)| i + 1))
    }

    /// The length of the `\w+` run (letters, digits, `_`) `s` starts with.
    fn word(s: &str) -> usize {
        s.find(|c: char| !(c.is_alphanumeric() || c == '_'))
            .unwrap_or(s.len())
    }

    /// Blanks (at least one), a cell, blanks (at least one), an instance,
    /// blanks, `(`, a body up to the first `;` with `)` before it: (cell,
    /// instance, body, the offset past the `;`), from line start `s`.
    fn instance_at(text: &str, s: usize) -> Option<(&str, &str, &str, usize)> {
        let rest = &text[s..];
        let at = rest.trim_start();
        if at.len() == rest.len() {
            return None;
        }
        let cell = &at[..word(at)];
        let gap = &at[cell.len()..];
        let at = gap.trim_start();
        let inst = &at[..word(at)];
        if cell.is_empty() || inst.is_empty() || at.len() == gap.len() {
            return None;
        }
        let at = at[inst.len()..].trim_start().strip_prefix('(')?;
        let semi = at.find(';')?;
        let body = at[..semi].strip_suffix(')').filter(|b| !b.is_empty())?;
        Some((cell, inst, body, text.len() - at.len() + semi + 1))
    }

    /// The net of each `.port(net)` in an instance's body, a leading `\`
    /// (an escaped Verilog name) left off.
    fn ports(body: &str) -> Vec<String> {
        let mut nets = Vec::new();
        let mut from = 0;
        while let Some(k) = body[from..].find('.') {
            let dot = from + k;
            match port_at(&body[dot + 1..]) {
                Some((net, len)) => {
                    nets.push(net.to_string());
                    from = dot + 1 + len;
                }
                None => from = dot + 1,
            }
        }
        nets
    }

    /// After a `.`: blanks, a port name, blanks, `(`, blanks, an optional
    /// `\`, the net (no blank, no `)`), blanks, `)`: (the net, the bytes
    /// read).
    fn port_at(s: &str) -> Option<(&str, usize)> {
        let at = s.trim_start();
        let name = word(at);
        if name == 0 {
            return None;
        }
        let at = at[name..].trim_start().strip_prefix('(')?.trim_start();
        let stop = |c: char| c.is_whitespace() || c == ')';
        let at = match at.strip_prefix('\\') {
            Some(r) if r.starts_with(|c: char| !stop(c)) => r,
            _ => at,
        };
        let net = &at[..at.find(stop).unwrap_or(at.len())];
        if net.is_empty() {
            return None;
        }
        let at = at[net.len()..].trim_start().strip_prefix(')')?;
        Some((net, s.len() - at.len()))
    }

    /// (liberty, SPICE cell models) of `$PDK_ROOT/<pdk>[A]/libs.ref/<pdk>/`;
    /// the liberty file prefers the `tt` corner.
    fn pdk_paths(pdk: &str) -> Result<(String, String), String> {
        let root = std::env::var("PDK_ROOT").map_err(|_| {
            "PDK_ROOT environment variable not set. Set it to your PDK installation root, \
             or provide explicit liberty= and spice_models= paths."
                .to_string()
        })?;
        let find = |sub: &str, ext: &str, prefer: &str| {
            ["A", ""].into_iter().find_map(|sfx| {
                let dir = std::fs::read_dir(format!("{root}/{pdk}{sfx}/libs.ref/{pdk}/{sub}")).ok()?;
                let mut files: Vec<PathBuf> = dir.flatten().map(|e| e.path()).filter(|p| p.extension().is_some_and(|e| e == ext)).collect();
                files.sort();
                let i = files.iter().position(|p| p.file_name().is_some_and(|n| n.to_string_lossy().contains(prefer))).unwrap_or(0);
                files.get(i).map(|p| p.to_string_lossy().into_owned())
            })
            .ok_or_else(|| {
                format!(
                    "Could not find .{ext} file for PDK '{pdk}' under $PDK_ROOT={root}. \
                     Searched: {root}/{pdk}A/libs.ref/{pdk}/{sub}/ and {root}/{pdk}/libs.ref/{pdk}/{sub}/"
                )
            })
        };
        Ok((find("lib", "lib", "tt")?, find("spice", "spice", "")?))
    }
}

/// Without the `backend` feature nothing runs OpenVAF: a Verilog-A source in
/// the load list is an error, a compiled `.osdi` passes through.
#[cfg(not(feature = "backend"))]
pub fn osdi_loads(loads: &[String]) -> Result<Vec<String>, String> {
    loads
        .iter()
        .map(|l| {
            if is_source(l) {
                Err(format!(
                    "{l}: compiling Verilog-A needs spicerack's `backend` feature"
                ))
            } else {
                Ok(l.clone())
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_vs_binary() {
        assert!(is_source("/m/a.va"));
        assert!(is_source("A.VA"));
        assert!(!is_source("/m/a.osdi"));
        assert!(!is_source("/m/va"));
    }

    #[test]
    fn binaries_pass_through_without_openvaf() {
        assert_eq!(
            osdi_loads(&["/m/a.osdi".into()]).unwrap(),
            vec!["/m/a.osdi".to_string()]
        );
    }

    #[cfg(feature = "backend")]
    #[test]
    fn inline_source_resolves_to_a_stable_file() {
        let src = "module t(a); inout a; electrical a; endmodule";
        let a = resolve(src).unwrap();
        assert_eq!(a, resolve(src).unwrap());
        assert_eq!(std::fs::read_to_string(&a).unwrap(), src);
    }

    /// Oracle: each caller's own result. Processes (pytest -n, nextest)
    /// lowering one inline block at once each get its `.vvp`; before the
    /// write-then-rename one could compile another's half-written source
    /// (batch-14, the `[espice]` twin's `verilog` cell; 16 callers failed
    /// in the second round of eight, 200 rounds pass).
    #[cfg(feature = "backend")]
    #[test]
    fn one_inline_block_lowered_at_once_compiles_for_every_caller() {
        let clock = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH);
        let stamp = clock.unwrap().as_nanos();
        for round in 0..8 {
            let block = VerilogBlock {
                source: format!(
                    "// {stamp} {round}\nmodule inv(input a, output y); assign y = ~a; endmodule\n"
                ),
                mode: VerilogMode::Simulate,
                instance_name: "u1".into(),
                connections: [("a".into(), VerilogConnection::Single("in".into()))].into(),
                pdk: None,
                liberty: None,
                spice_models: None,
            };
            let barrier = std::sync::Barrier::new(16);
            std::thread::scope(|s| {
                let runs: Vec<_> = (0..16)
                    .map(|_| {
                        s.spawn(|| {
                            barrier.wait();
                            lower(&block)
                        })
                    })
                    .collect();
                for run in runs {
                    if let Err(e) = run.join().unwrap() {
                        panic!("round {round}: {e}");
                    }
                }
            });
        }
    }

    #[cfg(feature = "backend")]
    #[test]
    fn yosys_netlist_cells() {
        let n = "module top(a, b, out);\n  input a;\n  sky130_fd_sc_hd__and2_1 _0_ ( .A(\\a ), .B(b), .X(\\out ) );\nendmodule\n";
        assert_eq!(
            run::gate_instances(n),
            vec![(
                "_0_".into(),
                "sky130_fd_sc_hd__and2_1".into(),
                vec!["a".into(), "b".into(), "out".into()]
            )]
        );
    }

    #[cfg(feature = "backend")]
    #[test]
    fn yosys_scans_capture_what_the_regexes_did() {
        // Each text, its module name and its cells as the `regex` 1.12.3
        // patterns these scans replace captured them (recorded from both
        // run side by side, 2026-11-08, before `regex` left).
        type Gates = Vec<(&'static str, &'static str, Vec<&'static str>)>;
        let cases: [(&str, Option<&str>, Gates); 23] = [
            (
                "module top(a, b, out);\n  input a;\n  sky130_fd_sc_hd__and2_1 _0_ ( .A(\\a ), .B(b), .X(\\out ) );\nendmodule\n",
                Some("top"),
                vec![("_0_", "sky130_fd_sc_hd__and2_1", vec!["a", "b", "out"])],
            ),
            ("", None, vec![]),
            ("module", None, vec![]),
            ("module x", Some("x"), vec![]),
            ("modulex y\nmodule  z(", Some("z"), vec![]),
            ("  // c\n\n   module\n\n  foo (a);", Some("foo"), vec![]),
            ("x module y\n\tmodule  w_1$ (a)", Some("w_1"), vec![]),
            ("\r\nmodule r\r\n", Some("r"), vec![]),
            ("\n\nmodule é_ü(x)", Some("é_ü"), vec![]),
            ("module ;\nmodule ok", Some("ok"), vec![]),
            ("/* */ module no\n module yes", Some("yes"), vec![]),
            (
                "  c1 i1 (.A(x), .B( \\y ), .C(\\ ), .D(z ) , .E (w)) ;\n",
                None,
                vec![],
            ),
            (
                "  c1 i1 (.A(x));\n  c2 i2 ( .Q(q) );",
                None,
                vec![("i1", "c1", vec!["x"]), ("i2", "c2", vec!["q"])],
            ),
            (
                "\nc3 i3 (.A(a));\nc4 i4 (.A(b));\n  \nc5 i5 (.A(c));",
                None,
                vec![("i3", "c3", vec!["a"]), ("i5", "c5", vec!["c"])],
            ),
            (
                "  wire w (x);\n  assign a (b);\n  input i (j);\n  reg r (s);\n",
                None,
                vec![],
            ),
            (
                "  c6 i6 ();\n  c7 i7 ( ) ;\n  c8 i8 (.A(a)) x;\n  c9 i9 (.A(a) ; );",
                None,
                vec![],
            ),
            (
                "  c10\n   i10\n (\n .A(\na\n)\n);\n",
                None,
                vec![("i10", "c10", vec!["a"])],
            ),
            (
                "  c11 i11 (.A(a), .B(b)); c12 i12 (.A(z));\n  c13 i13 (.A(y));",
                None,
                vec![("i11", "c11", vec!["a", "b"]), ("i13", "c13", vec!["y"])],
            ),
            (
                "  c14 i14 (. A ( \\x[0] ) , .B(\\\\b ), .C(c)d), .(e), .F(), .G(g h));",
                None,
                vec![("i14", "c14", vec!["x[0]", "\\b", "c"])],
            ),
            ("  c15 i15 (.A(a))\n  ;", None, vec![]),
            ("\t c16 i16 (\\a);", None, vec![("i16", "c16", vec![])]),
            (
                "  c17 \\i17 (.A(a));\n  c18 i18(.A(a));",
                None,
                vec![("i18", "c18", vec!["a"])],
            ),
            (
                "  c19 i19 (.A(a.b), .B(a .c));",
                None,
                vec![("i19", "c19", vec!["a.b"])],
            ),
        ];
        for (text, module, gates) in cases {
            assert_eq!(run::module_name(text), module, "{text:?}");
            let want: Vec<(String, String, Vec<String>)> = gates
                .into_iter()
                .map(|(i, c, n)| (i.into(), c.into(), n.into_iter().map(Into::into).collect()))
                .collect();
            assert_eq!(run::gate_instances(text), want, "{text:?}");
        }
    }
}
