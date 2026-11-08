//! The `catalogue!` macro: [`Code`](super::Code), `Code::ALL` and
//! [`info`](super::info) from one list of rows (`mod.rs`), so a code cannot
//! lack its row nor be missing from `ALL`.
//!
//! Cites: none.
//!
//! Contents:
//! - `macro_rules! catalogue`

/// Defines [`Code`], [`Code::ALL`] and [`info`] from one list of rows, so
/// a code cannot lack its row nor be missing from `ALL`.
macro_rules! catalogue {
    ($(
        $(#[$doc:meta])*
        $code:ident $kind:ident {
            title: $title:literal,
            cite: $cite:literal,
            explain: $explain:literal $(,)?
        }
    )*) => {
        /// A diagnostic's code.
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub enum Code {
            $($(#[$doc])* $code,)*
        }

        impl Code {
            /// Every code, in catalogue order.
            pub const ALL: &[Code] = &[$(Code::$code),*];
        }

        /// Code `c`'s row.
        pub const fn info(c: Code) -> Info {
            match c {
                $(Code::$code => Info {
                    title: $title,
                    cite: $cite,
                    explain: $explain,
                    kind: Kind::$kind,
                },)*
            }
        }
    };
}
