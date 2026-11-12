// pxl-core/sys/sched/phase.spec.rs
//
// GENERATED from spec/client/core/sys/sched/phase.json
//
// The core's tick phases, in the order they run (pxl-core sys/sched/tick.rs):
// what moves the eye first, unbudgeted and O(1); then what is visible. A
// phase's number is its place here, and a report of per-phase time is indexed
// by it.

// Names as the spec spells them, so one record has one name on every side.
// A table is generated whole, whether or not this crate calls an entry.
#![allow(
    non_upper_case_globals,
    non_snake_case,
    clippy::absolute_paths,
    non_camel_case_types,
    dead_code,
    reason = "generated from the spec"
)]

pub const PHASES: usize = 2;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Phase {
    interaction = 0,
    visible = 1,
}

impl Phase {
    /// Every member, in the JSON's own order.
    pub const ALL: [Phase; 2] = [Phase::interaction, Phase::visible];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<Phase> {
        Phase::ALL.into_iter().find(|member| *member as u8 == val)
    }
}
