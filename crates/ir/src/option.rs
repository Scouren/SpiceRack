//! Simulator state as items: options, the environment (temperature, nominal temperature), initial conditions and node sets.
//!
//! Owner: run (`rewrite/IR-KERNEL.md` §10, `rewrite/IR-RUN.md`). Definition phase stub
//! written by the kernel owner: the type names the kernel references, with placeholder bodies.
//!
//! Cites: `rewrite/IR-RUN.md`.
//!
//! Contents: `pub struct OptionRow`, `pub struct Env`, `pub struct NodeSet`

/// A simulator option.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OptionRow {} // run: to define

/// Temperature and nominal temperature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Env {} // run: to define

/// An initial condition or a node set.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NodeSet {} // run: to define

const _: () = assert!(size_of::<OptionRow>() == 0);
const _: () = assert!(size_of::<Env>() == 0);
const _: () = assert!(size_of::<NodeSet>() == 0);
