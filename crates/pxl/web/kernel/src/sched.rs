// pxl-kernel/sched.rs

//! The kernel's scheduling: the turn every layer takes, the order a tick runs
//! its steps in, and what each one cost. Dividing a turn is the mechanism's
//! (xpute_runtime::sched); what runs inside one is decided here.

pub mod ledger;
pub mod priority;
pub mod tick;
pub mod turn;
