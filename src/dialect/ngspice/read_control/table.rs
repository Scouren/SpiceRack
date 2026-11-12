//! The verdict word lists of `rewrite/S3-CONTROL.md` §2 (ngspice-45 `commands.c`), the
//! display variables and the frontend-only option keys: data the planner reads.
//!
//! Contents: `pub(super) const INERT`, `EFFECTS`, `FLOWS`, `OWNS`, `ANALYSES`, `DISPLAY_VARS`,
//! `FRONTEND_KEYS` (each `&[&str]`)

/// Display commands (`S3-CONTROL.md` §2: Inert).
pub(super) const INERT: &[&str] = &[
    "let",
    "reshape",
    "define",
    "deftype",
    "plot",
    "display",
    "destroy",
    "setplot",
    "setscale",
    "transpose",
    "gnuplot",
    "wrdata",
    "wrs2p",
    "hardcopy",
    "asciiplot",
    "write",
    "compose",
    "unlet",
    "remzerovec",
    "print",
    "esave",
    "eprint",
    "eprvcd",
    "edisplay",
    "cross",
    "undefine",
    "listing",
    "dump",
    "fft",
    "psd",
    "spec",
    "show",
    "showmod",
    "sysinfo",
    "state",
    "trace",
    "iplot",
    "status",
    "jobs",
    "bug",
    "where",
    "newhelp",
    "tutorial",
    "help",
    "oldhelp",
    "shift",
    "history",
    "echo",
    "rusage",
    "getcwd",
    "version",
    "diff",
    "rehash",
    "cdump",
    "mdump",
    "mrdump",
    "settype",
    "strcmp",
    "strstr",
    "strslice",
    "linearize",
    "cutout",
    "devhelp",
    "inventory",
    "wrnodev",
    "check_ifparm",
    "snsave",
];

/// Commands whose effect the IR cannot state (`S3-CONTROL.md` §2: Script).
pub(super) const EFFECTS: &[&str] = &[
    "snload",
    "circbyline",
    "alias",
    "unalias",
    "setcirc",
    "load",
    "edit",
    "mc_source",
    "resume",
    "stop",
    "delete",
    "step",
    "remcirc",
    "removecirc",
    "aspice",
    "rspice",
    "source",
    "shell",
    "cd",
    "fopen",
    "fread",
    "fclose",
];

/// Data-dependent flow words outside a block they open (`control.c`).
pub(super) const FLOWS: &[&str] = &["else", "break", "continue", "label", "goto"];

/// ngspice's own (`S3-CONTROL.md` §2: Private).
pub(super) const OWNS: &[&str] = &["setseed", "optran", "codemodel"];

/// The analysis commands (`commands.c:297-347`).
pub(super) const ANALYSES: &[&str] = &[
    "op", "tf", "tran", "pss", "sp", "ac", "dc", "pz", "sens", "disto", "noise",
];

/// Frontend variables that only change how results are shown or written (`INV-constructs.md`
/// K3, checked against `options.c` `cp_usrset`: none reaches `if_option`).
pub(super) const DISPLAY_VARS: &[&str] = &[
    "appendwrite",
    "askquit",
    "brief",
    "curplottitle",
    "gridstyle",
    "height",
    "noaskquit",
    "nobreak",
    "nolegend",
    "nomoremode",
    "nopadding",
    "noprintscale",
    "numdgt",
    "plotstyle",
    "pointchars",
    "width",
    "wr_singlescale",
    "wr_vecnames",
    "xbrushwidth",
    "xgridwidth",
];

/// Typed option keys that are frontend variables, not circuit options (absent from
/// `cktsopt.c:240-380`): a script `set` of one is global and survives `reset`.
pub(super) const FRONTEND_KEYS: &[&str] = &[
    "rseries",
    "scale",
    "seed",
    "autostop",
    "interp",
    "noacct",
    "warn",
    "maxwarns",
    "norefvalue",
    "noinit",
    "nfreqs",
    "fourgridsize",
];
