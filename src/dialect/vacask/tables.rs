//! VACASK's constant tables that more than one area reads: the builtin constants and their
//! documented values, the library files VACASK ships and their families (a `load` of one is a
//! `DeviceLibrary`, A12 C5, R17), and (S4.1) the instance and model parameter names of each module for
//! `Write::admits` (R21).
//!
//! Cites: VD expr-constants (the values); the store's `lib/vacask/mod` listing (36be560);
//! `rewrite/DIALECT-VACASK.md` §3.2 (load), §4.
//!
//! Contents: `CONSTANTS`, `SOURCE_MODULES`, `STATIC_FUNCS`, `DYNAMIC_LACKS`, `DYNAMIC_FUNCS`, `DYNAMIC_BINARY`,
//! `SPICE_FILES`, `OWN_FILES`, `library`, `default_file`, `function`, `draw`, `binary` (crate-private)

use crate::ir;

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

/// The functions VACASK has in a parameter expression (VD expr-functions).
pub(super) const STATIC_FUNCS: &[ir::Func] = &[
    ir::Func::Abs,
    ir::Func::Sqrt,
    ir::Func::Exp,
    ir::Func::Ln,
    ir::Func::Log10,
    ir::Func::Pow,
    ir::Func::Min,
    ir::Func::Max,
    ir::Func::Floor,
    ir::Func::Ceil,
    ir::Func::Trunc,
    ir::Func::ToInt,
    ir::Func::RoundHalfAway,
    ir::Func::Sin,
    ir::Func::Cos,
    ir::Func::Tan,
    ir::Func::Asin,
    ir::Func::Acos,
    ir::Func::Atan,
    ir::Func::Atan2,
    ir::Func::Sinh,
    ir::Func::Cosh,
    ir::Func::Tanh,
    ir::Func::Asinh,
    ir::Func::Acosh,
    ir::Func::Atanh,
    ir::Func::Hypot,
    ir::Func::SgnZeroPos,
    ir::Func::Fmod,
];

/// The functions VACASK's parameter language has and its behavioural sources lack (VD
/// dev-builtin-behavioral, Functions): an expression the IR classes dynamic cannot hold them.
/// ponytail: only a direct `$temp` makes it dynamic here; a parameter that reads one (R14's
/// transitive class) is not traced yet (S4.1, with the behavioural sources).
pub(super) const DYNAMIC_LACKS: &[&str] = &["round", "integer", "fmod"];

/// The functions and operators of VACASK's behavioural sources (VD dev-builtin-behavioral).
pub(super) const DYNAMIC_FUNCS: &[ir::Func] = &[
    ir::Func::Abs,
    ir::Func::Sqrt,
    ir::Func::Exp,
    ir::Func::Ln,
    ir::Func::Log10,
    ir::Func::Pow,
    ir::Func::Min,
    ir::Func::Max,
    ir::Func::Floor,
    ir::Func::Ceil,
    ir::Func::ToInt,
    ir::Func::Sin,
    ir::Func::Cos,
    ir::Func::Tan,
    ir::Func::Asin,
    ir::Func::Acos,
    ir::Func::Atan,
    ir::Func::Atan2,
    ir::Func::Sinh,
    ir::Func::Cosh,
    ir::Func::Tanh,
    ir::Func::Asinh,
    ir::Func::Acosh,
    ir::Func::Atanh,
    ir::Func::Hypot,
    ir::Func::SgnZeroPos,
    ir::Func::Ddt,
    ir::Func::Idt,
    ir::Func::ToReal,
];

/// The binary operators of VACASK's behavioural sources (no bit operators).
pub(super) const DYNAMIC_BINARY: &[ir::BinOp] = &[
    ir::BinOp::Add,
    ir::BinOp::Sub,
    ir::BinOp::Mul,
    ir::BinOp::DivTyped,
    ir::BinOp::Eq,
    ir::BinOp::Ne,
    ir::BinOp::Lt,
    ir::BinOp::Le,
    ir::BinOp::Gt,
    ir::BinOp::Ge,
    ir::BinOp::And,
    ir::BinOp::Or,
];

/// The modules whose instances the sources area reads (S4.2): VACASK's builtins (VD
/// cir-loading, Builtin devices) and `tline_ideal` (VD dev-builtin; DIALECT-VACASK §3.2).
pub(super) const SOURCE_MODULES: &[&str] = &[
    "vsource",
    "isource",
    "vcvs",
    "vccs",
    "ccvs",
    "cccs",
    "mutual",
    "tline_ideal",
];

/// The SPICE device files VACASK ships under `spice/` (and the `spice/sn/`, `spice/full/`
/// variants), by module file stem, and the family each implements (VD dev-spice). `diode` serves
/// levels 1 and 3; its library is stated as `Diode` (S4.1 settles the level-3 model's library).
pub(super) const SPICE_FILES: &[(&str, ir::ModelFamily)] = &[
    ("resistor", ir::ModelFamily::Resistor),
    ("capacitor", ir::ModelFamily::Capacitor),
    ("inductor", ir::ModelFamily::Inductor),
    ("diode", ir::ModelFamily::Diode),
    ("bjt", ir::ModelFamily::GummelPoon),
    ("jfet1", ir::ModelFamily::JfetShichman),
    ("jfet2", ir::ModelFamily::JfetParkerSkellern),
    ("mes1", ir::ModelFamily::Mesfet),
    ("mos1", ir::ModelFamily::Mos1),
    ("mos2", ir::ModelFamily::Mos2),
    ("mos3", ir::ModelFamily::Mos3),
    ("mos6", ir::ModelFamily::Mos6),
    ("mos9", ir::ModelFamily::Mos9),
    ("vdmos", ir::ModelFamily::Vdmos),
    ("bsim3v3", ir::ModelFamily::Bsim3),
    ("bsim4v8", ir::ModelFamily::Bsim4),
];

/// VACASK's own device files at the top of its module directory, and their families; the first
/// file of a family that has no `spice/` file is its default (VD dev-builtin, the store's
/// `lib/vacask/mod` listing). `tline_ideal.osdi` is the ideal line's family (R39, A20);
/// `opamp.osdi` implements no equation family: a `load` of it is a user module's.
pub(super) const OWN_FILES: &[(&str, ir::ModelFamily)] = &[
    ("psp103v4.osdi", ir::ModelFamily::Psp103),
    ("bsimbulk106.osdi", ir::ModelFamily::BsimBulk),
    ("vbic_1p3.osdi", ir::ModelFamily::Vbic),
    ("vbic_1p3_4t.osdi", ir::ModelFamily::Vbic),
    ("vbic_1p3_5t.osdi", ir::ModelFamily::Vbic),
    ("resistor.osdi", ir::ModelFamily::Resistor),
    ("capacitor.osdi", ir::ModelFamily::Capacitor),
    ("inductor.osdi", ir::ModelFamily::Inductor),
    ("diode.osdi", ir::ModelFamily::Diode),
    ("bsim3v3.osdi", ir::ModelFamily::Bsim3),
    ("bsim4v8.osdi", ir::ModelFamily::Bsim4),
    ("tline_ideal.osdi", ir::ModelFamily::TlineIdeal),
];

/// The family the library file `path` (as a `load` writes it) implements, or `None` for any
/// other file.
pub(super) fn library(path: &str) -> Option<ir::ModelFamily> {
    let spice = ["spice/", "spice/sn/", "spice/full/"]
        .iter()
        .filter_map(|d| path.strip_prefix(d)?.strip_suffix(".osdi"))
        .find_map(|stem| {
            SPICE_FILES
                .iter()
                .find(|(s, _)| *s == stem)
                .map(|&(_, f)| f)
        });
    spice.or_else(|| OWN_FILES.iter().find(|(f, _)| *f == path).map(|&(_, f)| f))
}

/// The file VACASK loads `family` from by default: its `spice/` file, else its first own file.
pub(super) fn default_file(family: ir::ModelFamily) -> Option<String> {
    let spice = SPICE_FILES.iter().find(|(_, f)| *f == family);
    match spice {
        Some((stem, _)) => Some(format!("spice/{stem}.osdi")),
        None => OWN_FILES
            .iter()
            .find(|(_, f)| *f == family)
            .map(|(p, _)| (*p).to_string()),
    }
}

/// The function a VACASK call name reads as, with its argument count.
pub(super) fn function(name: &str) -> Option<(ir::Func, usize)> {
    use ir::Func as F;
    Some(match name {
        "sin" => (F::Sin, 1),
        "cos" => (F::Cos, 1),
        "tan" => (F::Tan, 1),
        "asin" => (F::Asin, 1),
        "acos" => (F::Acos, 1),
        "atan" => (F::Atan, 1),
        "atan2" => (F::Atan2, 2),
        "sinh" => (F::Sinh, 1),
        "cosh" => (F::Cosh, 1),
        "tanh" => (F::Tanh, 1),
        "asinh" => (F::Asinh, 1),
        "acosh" => (F::Acosh, 1),
        "atanh" => (F::Atanh, 1),
        "exp" => (F::Exp, 1),
        "log" | "ln" => (F::Ln, 1),
        "log10" => (F::Log10, 1),
        "sqrt" => (F::Sqrt, 1),
        "pow" => (F::Pow, 2),
        "hypot" => (F::Hypot, 2),
        "abs" => (F::Abs, 1),
        "floor" => (F::Floor, 1),
        "ceil" => (F::Ceil, 1),
        "round" => (F::RoundHalfAway, 1),
        "integer" => (F::Trunc, 1),
        "int" => (F::ToInt, 1),
        "fmod" => (F::Fmod, 2),
        "min" => (F::Min, 2),
        "max" => (F::Max, 2),
        "ddt" => (F::Ddt, 1),
        "idt" => (F::Idt, 1),
        _ => return None,
    })
}

/// The draw a VACASK call name reads as.
pub(super) fn draw(name: &str) -> Option<ir::Dist> {
    Some(match name {
        "agauss" => ir::Dist::AbsGauss,
        "gauss" => ir::Dist::RelGauss,
        "aunif" => ir::Dist::AbsUnif,
        "unif" => ir::Dist::RelUnif,
        _ => return None,
    })
}

/// The binary operator token `op` spells (`**` is `Pow`, handled by the caller).
pub(super) fn binary(op: &str) -> Option<ir::BinOp> {
    use ir::BinOp as B;
    Some(match op {
        "+" => B::Add,
        "-" => B::Sub,
        "*" => B::Mul,
        "/" => B::DivTyped,
        "==" => B::Eq,
        "!=" => B::Ne,
        "<" => B::Lt,
        "<=" => B::Le,
        ">" => B::Gt,
        ">=" => B::Ge,
        "&&" => B::And,
        "||" => B::Or,
        "&" => B::BitAnd,
        "|" => B::BitOr,
        "^" => B::BitXor,
        "<<" => B::Shl,
        ">>" => B::Shr,
        _ => return None,
    })
}
