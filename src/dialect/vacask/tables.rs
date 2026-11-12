//! VACASK's constant tables that more than one area reads: the builtin constants and their
//! documented values, the library files VACASK ships (a `load` of one is the device library,
//! A12 C5), and (S4.1) the instance and model parameter names of each module for
//! `Write::admits` (R21).
//!
//! Cites: VD expr-constants (the values); the store's `lib/vacask/mod` listing (36be560);
//! `rewrite/DIALECT-VACASK.md` §3.2 (load), §4.
//!
//! Contents: `CONSTANTS`, `LIBRARY_FILES`, `is_library_file` (crate-private)

/// `(name, value as VACASK documents it)`.
pub(super) const CONSTANTS: &[(&str, &str)] = &[
    ("M_E", "2.718281828459045"),
    ("M_LOG2E", "1.4426950408889634"),
    ("M_LOG10E", "0.4342944819032518"),
    ("M_LN2", "0.6931471805599453"),
    ("M_LN10", "2.302585092994046"),
    ("M_PI", "3.141592653589793"),
    ("M_TWO_PI", "6.283185307179586"),
    ("M_PI_2", "1.5707963267948966"),
    ("M_PI_4", "0.7853981633974483"),
    ("M_1_PI", "0.3183098861837907"),
    ("M_2_PI", "0.6366197723675814"),
    ("M_2_SQRTPI", "1.1283791670955126"),
    ("M_SQRT2", "1.4142135623730951"),
    ("M_SQRT1_2", "0.7071067811865475"),
    ("M_DEGPERRAD", "57.29577951308232"),
    ("P_Q", "1.6021918e-19"),
    ("P_C", "299792456.2"),
    ("P_K", "1.3806226e-23"),
    ("P_H", "6.6260755e-34"),
    ("P_EPS0", "8.854187924e-12"),
    ("P_U0", "1.2566370614359173e-06"),
    ("P_CELSIUS0", "273.15"),
];

/// The library files under VACASK's module directory, by the path a `load` names them with.
pub(super) const LIBRARY_FILES: &[&str] = &[
    "resistor.osdi",
    "capacitor.osdi",
    "inductor.osdi",
    "diode.osdi",
    "bsim3v3.osdi",
    "bsim4v8.osdi",
    "bsimbulk106.osdi",
    "psp103v4.osdi",
    "vbic_1p3.osdi",
    "vbic_1p3_4t.osdi",
    "vbic_1p3_5t.osdi",
    "tline_ideal.osdi",
    "opamp.osdi",
];

/// Whether `path` (as a `load` writes it) names a library file VACASK ships: one of
/// [`LIBRARY_FILES`], or a file under `spice/`, `spice/sn/` or `spice/full/`.
pub(super) fn is_library_file(path: &str) -> bool {
    LIBRARY_FILES.contains(&path)
        || ["spice/", "spice/sn/", "spice/full/"].iter().any(|d| {
            path.strip_prefix(d)
                .is_some_and(|f| !f.contains('/') && f.ends_with(".osdi"))
        })
}
