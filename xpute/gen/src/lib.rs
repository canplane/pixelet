// xpute-gen/lib.rs

//! xpute's generator. `spec` generates the code that data specs describe, and
//! writes or checks every generator's files, the link's (xpute-link-gen) too;
//! `fmt` runs each generated file through the formatter its tree checks it
//! with.

pub mod fmt;
pub mod spec;
