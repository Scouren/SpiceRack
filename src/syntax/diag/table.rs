//! The `catalogue!` macro: [`Code`](super::Code), `Code::ALL` and
//! [`info`](super::info) from one list of rows (the `rows_*.rs` blocks,
//! chained in `mod.rs`), so a code cannot lack its row nor be missing from
//! `ALL`; `catalogue_end!` ends that chain.
//!
//! Cites: none.
//!
//! Contents:
//! - `macro_rules! catalogue`
//! - `macro_rules! catalogue_end`

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

/// The last link of the rows chain (`rows_lp!` … `rows_w_def!`): every
/// block's rows, accumulated after the `;`, become one `catalogue!` list.
macro_rules! catalogue_end {
    ( ; $($acc:tt)*) => {
        catalogue! { $($acc)* }
    };
}
