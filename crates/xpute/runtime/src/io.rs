// xpute-runtime/io.rs

//! The guest's I/O.
//!
//! What ran here was a queue that ordered demands, spent credits on them and
//! aborted what stopped being wanted. Ordering and credits are a policy, and
//! a policy that has to know the camera and the turn is not this module's —
//! it is the guest's, and it lives in one place there (pxl-kernel io/sched.rs)
//! rather than in a mechanism here and a caller above it.
