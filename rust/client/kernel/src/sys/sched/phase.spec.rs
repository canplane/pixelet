// pxl-kernel/sys/sched/phase.spec.rs
//
// GENERATED from spec/client/kernel/sys/sched/phase.json
//
// The kernel's tick phases, in the order they run on xpute's tick (xpute-guest
// sched/tick.rs): what moves the eye first, unbudgeted and O(1); then what is
// visible, which holds back the content's share (pxl-kernel sys/sched/param.rs
// `CONTENT_SHARE`); then content, then the cosmetic and the report. A phase's
// number is its place here, and a report of per-phase time is indexed by it.

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

pub const PHASES: usize = 5;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Phase {
    interaction = 0,
    visible = 1,
    content = 2,
    cosmetic = 3,
    report = 4,
}

impl Phase {
    /// Every member, in the JSON's own order.
    pub const ALL: [Phase; 5] = [
        Phase::interaction,
        Phase::visible,
        Phase::content,
        Phase::cosmetic,
        Phase::report,
    ];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<Phase> {
        Phase::ALL.into_iter().find(|member| *member as u8 == val)
    }
}
