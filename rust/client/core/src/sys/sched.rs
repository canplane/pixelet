// pxl-core/sys/sched.rs

//! How the core spends a turn: the edge that bounds the turn's tasks, the
//! phases of its tick and the share of the budget each holds back, when the
//! GPU can next take a frame, and the wall clock time the runtime tells it.

pub mod clock;
pub mod edge;
#[path = "sched/pacing.spec.rs"]
pub mod pacing_spec;
pub mod param;
#[path = "sched/phase.spec.rs"]
pub mod phase_spec;
pub mod queue;
pub mod tick;
