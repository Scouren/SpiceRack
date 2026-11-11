//! Distributed lines: nets and a model → an `O` (LTRA), `Y` (TXL), `P`
//! (CPL) or `U` (URC) card, as methods of Subcircuit. Cites ngspice manual
//! §6.2 (`OXXXXXXX n1 n2 n3 n4 mname`), §6.3 (`UXXXXXXX n1 n2 n3 mname
//! l=len <n=lumps>`), §6.4.1 (`YXXXXXXX N1 0 N2 0 mname <LEN=LENGTH>`) and
//! §6.4.2 (`PXXXXXXX NI1 ... GND1 NO1 ... GND2 mname <LEN=LENGTH>`).
//!
//! Contents:
//! - `macro_rules! pymethods_with_lines` (scoped to `python` by `#[macro_use]`)
//! - `pub(super) fn len_word(length: Option<Num>) -> String`

use super::args::Num;

/// `pymethods_with_waves! { #[pymethods] impl $ty { ... } }` with the four
/// line elements appended: pyo3 takes one `#[pymethods]` per class without
/// its `multiple-pymethods` feature, so this file adds its methods to the
/// one block in `subcircuit.rs`, where every name in them resolves
/// (`nets`, `push`, `Num`, `PyValueError`). The caller writes the
/// `#[pymethods]`, as `pymethods_with_waves!` asks.
macro_rules! pymethods_with_lines {
    ($hash:tt $attr:tt impl $ty:ty { $($body:tt)* }) => {
        pymethods_with_waves! {
            $hash $attr
            impl $ty {
                $($body)*

                /// A lossy line (LTRA), `O<name> in+ in- out+ out- model`: the
                /// line is its `ltra` model (`r`, `l`, `g`, `c` per unit
                /// length, and `len`).
                #[pyo3(signature = (name, input_positive, input_negative, output_positive, output_negative, *, model))]
                fn O(&mut self, name: &str, input_positive: &str, input_negative: &str, output_positive: &str, output_negative: &str, model: &str) {
                    let nets = self.nets(&[input_positive, input_negative, output_positive, output_negative]);
                    self.push(name, format!("O{} {} {}", name, nets, model));
                }

                /// A single lossy line (TXL), `Y<name> in+ in- out+ out- model
                /// [LEN=length]`: `length` replaces the `txl` model's.
                #[pyo3(signature = (name, input_positive, input_negative, output_positive, output_negative, *, model, length=None))]
                fn Y(&mut self, name: &str, input_positive: &str, input_negative: &str, output_positive: &str, output_negative: &str, model: &str, length: Option<Num>) {
                    let nets = self.nets(&[input_positive, input_negative, output_positive, output_negative]);
                    self.push(name, format!("Y{} {} {}{}", name, nets, model, super::lines::len_word(length)));
                }

                /// Coupled lines (CPL), `P<name> in1 ... ground_in out1 ...
                /// ground_out model [LEN=length]`: one output per input, in
                /// the same order; `length` replaces the `cpl` model's.
                #[pyo3(signature = (name, inputs, ground_in, outputs, ground_out, *, model, length=None))]
                fn P(&mut self, name: &str, inputs: Vec<String>, ground_in: &str, outputs: Vec<String>, ground_out: &str, model: &str, length: Option<Num>) -> PyResult<()> {
                    if inputs.is_empty() || inputs.len() != outputs.len() {
                        return Err(PyValueError::new_err(format!(
                            "P{}: {} inputs and {} outputs: coupled lines have one output per input, and at least one",
                            name,
                            inputs.len(),
                            outputs.len()
                        )));
                    }
                    let mut nets: Vec<&str> = inputs.iter().map(String::as_str).collect();
                    nets.push(ground_in);
                    nets.extend(outputs.iter().map(String::as_str));
                    nets.push(ground_out);
                    let nets = self.nets(&nets);
                    self.push(name, format!("P{} {} {}{}", name, nets, model, super::lines::len_word(length)));
                    Ok(())
                }

                /// A uniform distributed RC line (URC), `U<name> n1 n2 n3 model
                /// l=length [n=lumps]`: `n1` and `n2` are the line's ends, `n3`
                /// the node its capacitance goes to.
                #[pyo3(signature = (name, n1, n2, n3, *, model, length, lumps=None))]
                fn U(&mut self, name: &str, n1: &str, n2: &str, n3: &str, model: &str, length: Num, lumps: Option<u32>) {
                    let nets = self.nets(&[n1, n2, n3]);
                    let lumps = lumps.map_or(String::new(), |n| format!(" n={}", n));
                    self.push(name, format!("U{} {} {} l={}{}", name, nets, model, length.text(), lumps));
                }
            }
        }
    };
}

/// ` LEN=<length>`, the word a `Y` or `P` card ends with (a number in
/// plain decimal, an expression braced), or nothing when the card leaves
/// the length to its model.
pub(super) fn len_word(length: Option<Num>) -> String {
    length.map_or(String::new(), |l| format!(" LEN={}", l.text()))
}
