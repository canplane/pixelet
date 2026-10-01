// pxl-kernel/io.rs

//! The kernel's I/O: what it asks the host to read, in what order, and what
//! becomes of what arrives.
//!
//! What is wanted is the producers'; what is out, within the credits, and
//! what is held back after a failure is `sched`'s, the mechanism and the only
//! authority over them; where each want sits in one order, when a running
//! read gives way and how long a failure waits is `policy`'s, which `sched`
//! asks. What a body decodes into is `fetch`'s.

pub mod demand;
pub mod fetch;
pub mod file;
pub mod load;
pub mod policy;
pub mod sched;
