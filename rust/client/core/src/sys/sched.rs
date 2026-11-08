// pxl-core/sys/sched.rs

//! A turn's mechanics: its edge, its phases and what each holds back, and
//! the wall clock it is told.

pub mod clock;
pub mod edge;
#[path = "sched/pacing.spec.rs"]
pub mod pacing_spec;
pub mod param;
#[path = "sched/phase.spec.rs"]
pub mod phase_spec;
pub mod tick;
