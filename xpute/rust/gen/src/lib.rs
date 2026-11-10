// xpute-gen/lib.rs

//! xpute's generator. `spec` generates, for each language, the code a program's
//! machine and protocols describe; `fmt` runs each generated file through the
//! formatter its tree checks it with.

pub mod fmt;
pub mod spec;
