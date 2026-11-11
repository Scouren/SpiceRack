//! Deck keywords → closed enums: each keyword set's `name()` is the word
//! a deck spells, `FromStr` reads it in any case where the text enters
//! (anything else fails, naming the choices).
//!
//! Cites: ngspice manual §3.3.15 (a switch's `ON`/`OFF`), §11.3.6 (`.pz` types), §4.1.8 (TRRANDOM types);
//! Spectre Reference 19.1 `[REF]` (sweep `log`, the analysis keywords).
//!
//! Contents:
//! - keyword enums `Sweep`, `PzTransfer`, `PzKind`, `SpectreAnalysis`, `RandomKind`, `SwitchState`, each with
//!   `::ALL: &[Self]`, `::name(self) -> &'static str`, `FromStr`

/// A closed set of deck keywords: `name()` is the keyword, `FromStr` reads
/// it in any case (anything else fails, naming the choices), so the text is
/// parsed once, where it enters.
macro_rules! keywords {
    ($(#[$m:meta])* $name:ident, $what:literal { $($(#[$vm:meta])* $v:ident = $s:literal),+ $(,)? }) => {
        $(#[$m])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub(super) enum $name { $($(#[$vm])* $v),+ }

        impl $name {
            /// Every keyword, in declaration order.
            pub(super) const ALL: &'static [$name] = &[$($name::$v),+];

            /// The keyword as a deck spells it.
            pub(super) fn name(self) -> &'static str {
                match self { $($name::$v => $s),+ }
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            /// Reads a [`Self::name`] in any case.
            fn from_str(s: &str) -> Result<Self, String> {
                Self::ALL.iter().copied().find(|k| k.name().eq_ignore_ascii_case(s)).ok_or_else(|| {
                    let names: Vec<&str> = Self::ALL.iter().map(|k| k.name()).collect();
                    let (last, rest) = names.split_last().expect("a keyword set is never empty");
                    format!("unknown {} '{s}' (expected {} or {last})", $what, rest.join(", "))
                })
            }
        }
    };
}

keywords! {
    /// How a sweep spaces its points: a frequency sweep, or a `.step`.
    Sweep, "sweep" {
        Dec = "dec",
        Oct = "oct",
        Lin = "lin",
        /// N points in total, log-spaced: Spectre's `log=N-1` (`[REF]` p.43, steps).
        Log = "log",
    }
}

keywords! {
    /// What a `.pz` transfer function relates (ngspice manual §11.3.6).
    PzTransfer, "transfer type" {
        Vol = "vol",
        Cur = "cur",
    }
}

keywords! {
    /// What a `.pz` finds: poles, zeros, or both.
    PzKind, "pole-zero type" {
        Pol = "pol",
        Zer = "zer",
        Pz = "pz",
    }
}

keywords! {
    /// The analysis a Spectre `sweep` or `montecarlo` runs inside it, as
    /// its statement keyword.
    SpectreAnalysis, "Spectre analysis" {
        Dc = "dc",
        Ac = "ac",
        Tran = "tran",
        Noise = "noise",
        Xf = "xf",
        Sp = "sp",
        Stb = "stb",
        Pss = "pss",
        Pac = "pac",
        Pnoise = "pnoise",
        Pxf = "pxf",
        Pstb = "pstb",
        Hb = "hb",
    }
}

keywords! {
    /// TRRANDOM's distribution; its `TYPE` number is its place, from 1.
    RandomKind, "random type" {
        Uniform = "uniform",
        Gaussian = "gaussian",
        Exponential = "exponential",
        Poisson = "poisson",
    }
}

keywords! {
    /// A switch's state when its control starts inside the hysteresis
    /// band: the word after its model (ngspice manual §3.3.15).
    SwitchState, "switch state" {
        On = "ON",
        Off = "OFF",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keywords_read_in_any_case_and_name_the_choices() {
        assert_eq!("DEC".parse::<Sweep>(), Ok(Sweep::Dec));
        assert_eq!(
            "decade".parse::<Sweep>(),
            Err("unknown sweep 'decade' (expected dec, oct, lin or log)".into())
        );
        assert_eq!(RandomKind::ALL.len(), 4);
    }
}
