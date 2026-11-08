//! A closed set of kinds, declared once → its `enum` and `ALL`, every
//! variant in discriminant order. A new kind is one line in its list: the
//! enum and `ALL` cannot disagree, so `from_u32` (which reads `ALL`) never
//! misses a variant, and a table indexed by the kind can pin its length to
//! `ALL.len()`.
//!
//! Rung 5 (codebase-clean-code LADDER): a `macro_rules!` that expands to
//! the items written by hand before it (an enum and a `const` array), so
//! it leaves no code of its own.
//!
//! Imported with `#[macro_use]` (as `diag`'s `catalogue!`): `syntax`'s
//! module list declares this module first, so every module after it sees
//! the macro.
//!
//! Cites: none.
//!
//! Contents:
//! - `macro_rules! kinds`: `$name::COUNT: usize`, `$name::ALL: [$name; $name::COUNT]`

/// Declares `enum $name` with the given attributes and unit variants (each
/// followed by a comma), `$name::COUNT` and `$name::ALL`, every variant in
/// declaration (discriminant) order.
macro_rules! kinds {
    (
        $(#[$meta:meta])*
        $vis:vis enum $name:ident {
            $($(#[$vmeta:meta])* $variant:ident,)*
        }
    ) => {
        $(#[$meta])*
        $vis enum $name {
            $($(#[$vmeta])* $variant,)*
        }

        impl $name {
            /// How many kinds there are.
            pub const COUNT: usize = [$(stringify!($variant)),*].len();
            /// Every kind, in discriminant order.
            pub const ALL: [$name; $name::COUNT] = [$($name::$variant),*];
        }
    };
}
