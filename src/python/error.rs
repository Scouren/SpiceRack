//! Failures and notes as Python reads them (ZERO D14): a file's
//! diagnostics located in its source ([`PyDiagnostic`], Python
//! `Diagnostic`) → a written deck's notes ([`Wrote`]), or the one
//! exception every SpiceRack failure raises, `spicerack.Error` (a
//! `RuntimeError`), with a `status` and its `diagnostics`.
//!
//! `status` is `"refused"` when nothing was run: the writer does not write
//! the deck for the target, or no simulator is routed for it (`W0180` to
//! `W0182`). It is `"failed"` for everything else: a simulator, a compiler
//! or a hook that ran and failed.
//!
//! Cites: none (a `Diagnostic`'s fields are those of one entry of the
//! `spicerack.diagnostics/1` file, ZERO D14).
//!
//! Contents:
//! - `pub struct Error` (Python `spicerack.Error`)
//! - `pub(super) struct PyDiagnostic` (Python `Diagnostic`)
//! - `pub(super) struct Wrote`
//! - `pub(super) fn locate(diags: &Diags, src: &Source) -> Vec<PyDiagnostic>`
//! - `pub(super) fn refused(diags: &Diags, src: &Source) -> PyErr`, `pub(super) fn failed(e: impl Display) -> PyErr`,
//!   `pub(super) fn coded(status: &'static str, code: Code, text: String) -> PyErr`
//! - `pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()>`

use std::fmt::Display;

use pyo3::exceptions::PyRuntimeError;
use pyo3::prelude::*;

use crate::syntax::ast::{Node, Tag};
use crate::syntax::diag::{Code, Kind, info, messages};
use crate::syntax::render::{Diags, Source};
use crate::syntax::token;

pyo3::create_exception!(
    spicerack,
    Error,
    PyRuntimeError,
    "A SpiceRack failure: `status` (\"refused\" or \"failed\") and `diagnostics`."
);

// One diagnostic, as one entry of the `spicerack.diagnostics/1` file: its
// `code` and the code's fixed `kind` ("refusal" or "note"), `title` and
// `cite`; `message`, the line SpiceRack prints for it; and where it is in
// the deck's source: `line` and `column` (from 1), the `statement`'s text
// and, when the statement is an element, the `element`'s name. Those four
// are `None` for a diagnostic at no statement (a routing refusal, an
// analysis that is in no deck yet). `file` is `None`: a deck built in
// this process is not a file.
#[pyclass(name = "Diagnostic", module = "spicerack", get_all, frozen, eq)]
#[derive(Clone, Debug, PartialEq)]
pub(super) struct PyDiagnostic {
    code: String,
    kind: String,
    title: String,
    cite: String,
    pub(super) message: String,
    file: Option<String>,
    line: Option<u32>,
    column: Option<u32>,
    statement: Option<String>,
    element: Option<String>,
}

/// A deck written for a target: its text and the notes on it.
#[derive(Debug)]
pub(super) struct Wrote {
    pub(super) text: String,
    pub(super) notes: Vec<PyDiagnostic>,
}

/// `diags` (one file's: `src`'s) as Python reads them, in their order:
/// each with its message (`diag::messages`), its line and column in the
/// source, the text of the statement that holds its token (as written,
/// trimmed) and, when that statement is an element, the element's name.
///
/// # Panics
/// When `diags` is not `src`'s: a token past its tokens.
pub(super) fn locate(diags: &Diags, src: &Source) -> Vec<PyDiagnostic> {
    use token::Tag::{EndOfStatement, LangSpectre, LangSpice};
    let (t, tags) = (src.tokens, src.tokens.tags());
    // The structure tokens (`token.rs`): a statement's tokens lie between
    // two of them.
    let ends = |i: &usize| matches!(tags[*i], EndOfStatement | LangSpice | LangSpectre);
    let mut out = Vec::with_capacity(diags.diags.len());
    // ponytail: the tokens and nodes are scanned once per diagnostic (a
    // refusal's path); a statement index if a refused deck of 1e5
    // statements is ever slow.
    for d in &diags.diags {
        let at = d.token as usize;
        assert!(at < tags.len(), "a diagnostic on a token of `src`");
        // Its own line of `diags.messages` (a plugin's message may hold
        // a line break, so the lines are not counted off).
        let message = messages(std::slice::from_ref(d), t, src.name);
        // From the token after the structure token before it through the
        // one that ends it (zero width, at the end of its last line).
        let first = (0..at).rev().find(ends).map_or(0, |i| i + 1) as u32;
        let last = (at..tags.len()).find(ends).unwrap_or(tags.len() - 1) as u32;
        let text = &t.source()[t.start(first) as usize..t.end(last) as usize];
        let element = (0..src.ast.len() as u32).map(Node).find(|&n| {
            src.ast.tag(n) == Tag::Instance && (first..=d.token).contains(&src.ast.main_token(n))
        });
        let place = t.location(d.token);
        out.push(PyDiagnostic {
            line: Some(place.line),
            column: Some(place.column),
            statement: Some(text.trim().to_string()),
            element: element.map(|n| t.text(src.ast.main_token(n)).to_string()),
            ..PyDiagnostic::coded(d.code, message.trim_end().to_string())
        });
    }
    out
}

/// `spicerack.Error` for `diags`, a refusal of `src`: status `"refused"`,
/// its messages as the text, and every diagnostic of the file located,
/// the notes among them.
pub(super) fn refused(diags: &Diags, src: &Source) -> PyErr {
    let text = diags.messages.trim_end().to_string();
    raise("refused", text, locate(diags, src))
}

/// `spicerack.Error` for any other failure, carrying `e`'s text: status
/// `"failed"`, no diagnostics.
pub(super) fn failed(e: impl Display) -> PyErr {
    raise("failed", e.to_string(), Vec::new())
}

/// `spicerack.Error` for a failure with a catalogue code and no statement
/// (a routing refusal, a program that is not installed, an analysis that
/// is in no deck yet): `text`, and one diagnostic, `code`'s, whose message
/// is `text`.
pub(super) fn coded(status: &'static str, code: Code, text: String) -> PyErr {
    let found = vec![PyDiagnostic::coded(code, text.clone())];
    raise(status, text, found)
}

/// Adds `Error` and `Diagnostic` to module `m`.
pub(super) fn add(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("Error", m.py().get_type::<Error>())?;
    m.add_class::<PyDiagnostic>()
}

// ---- private ----

impl PyDiagnostic {
    /// `code` with its fixed text and `message`, at no statement.
    fn coded(code: Code, message: String) -> PyDiagnostic {
        let fixed = info(code);
        let kind = match fixed.kind {
            Kind::Note => "note",
            Kind::Refusal => "refusal",
        };
        PyDiagnostic {
            code: format!("{code:?}"),
            kind: kind.into(),
            title: fixed.title.into(),
            cite: fixed.cite.into(),
            message,
            file: None,
            line: None,
            column: None,
            statement: None,
            element: None,
        }
    }
}

/// `spicerack.Error(text)` with its `status` and `diagnostics` set.
fn raise(status: &'static str, text: String, found: Vec<PyDiagnostic>) -> PyErr {
    assert!(matches!(status, "refused" | "failed"), "a status D14 names");
    let err = Error::new_err(text);
    let attached = Python::attach(|py| -> PyResult<()> {
        let value = err.value(py);
        value.setattr("status", status)?;
        value.setattr("diagnostics", found)
    });
    // A failure to attach is raised in the error's place, never dropped.
    match attached {
        Ok(()) => err,
        Err(failed) => failed,
    }
}

// ---- Python methods ----

#[pymethods]
impl PyDiagnostic {
    fn __repr__(&self) -> String {
        format!("Diagnostic({:?})", self.message)
    }
}
