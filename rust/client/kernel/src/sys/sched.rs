// pxl-kernel/sys/sched.rs

//! A turn's mechanics: its edge and its phases, and the wall clock it is
//! told.

pub mod clock;
pub mod edge;
pub mod param;
pub mod tick;

#[path = "sched/pacing.spec.rs"]
pub mod pacing_spec;

#[path = "sched/phase.spec.rs"]
pub mod phase_spec;
