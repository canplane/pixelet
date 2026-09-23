// xpute-runtime/io.rs

//! The guest's I/O.
//!
//! What ran here was a queue that ordered demands, spent credits on them and
//! aborted what stopped being wanted. Ordering is a policy, and a policy that
//! has to know what the program is for is not a runtime's — it is the guest's,
//! and it belongs in one place there rather than split between a mechanism
//! here and a caller above it.
//!
//! What is left of the arrangement is the grant: the host says how many reads
//! may be out this turn, and the guest says which.
