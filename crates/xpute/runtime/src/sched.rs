// xpute-runtime/sched.rs

//! The guest's scheduler: the turn's quota as a deadline, its phases, its
//! passes and its tasks. The host's end of the turn is
//! packages/xpute/runtime/src/sched.

pub mod edge;
pub mod frame_budget;
pub mod quantum;
pub mod task;
pub mod tick;
