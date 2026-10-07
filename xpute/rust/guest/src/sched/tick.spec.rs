// xpute-guest/sched/tick.spec.rs
//
// GENERATED from spec/sched/tick.json — do not edit.
//
// A tick's phases, in the order they run and are prioritized: interaction unbudgeted,
// then what is visible, then content, then the cosmetic and the report. A phase's number
// is its place here, and a report of per-phase time is indexed by it.

// Casing here is the spec's and not Rust's: a name is spelled as the JSON
// spells it, so that one record is indexed by one name on both sides. To
// case them the way Rust would is to rename the protocol.
//
// `dead_code` because a table is generated whole: an entry earns its
// place in the JSON, not by having a caller in this crate.
#![allow(non_upper_case_globals, non_snake_case, non_camel_case_types, dead_code)]

pub const PHASES: &[&str] = &["interaction", "visible", "content", "cosmetic", "report"];
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
    pub const ALL: [Phase; 5] = [Phase::interaction, Phase::visible, Phase::content, Phase::cosmetic, Phase::report];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(value: u8) -> Option<Phase> {
        Phase::ALL.into_iter().find(|member| *member as u8 == value)
    }
}
