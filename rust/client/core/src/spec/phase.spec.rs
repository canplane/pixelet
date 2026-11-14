// pxl-core/spec/phase.spec.rs
//
// GENERATED from spec/client/core/sys/sched/phase.json (xpute-gen data.rs)
//
// The core's tick phases, in the order they run (pxl-core sys/sched/tick.rs):
// what moves the eye first, unbudgeted and O(1); then what is visible. A
// phase's number is its place here, and a report of per-phase time is indexed
// by it.

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Phase {
    Interaction = 0,
    Visible = 1,
}

impl Phase {
    /// Every member, in its source's own order.
    pub const ALL: [Phase; 2] = [Phase::Interaction, Phase::Visible];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Phase> {
        match val {
            0 => Some(Phase::Interaction),
            1 => Some(Phase::Visible),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Phase::Interaction => "interaction",
            Phase::Visible => "visible",
        }
    }
}
