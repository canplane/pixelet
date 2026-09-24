// pxl-kernel/layer/decor/kind.rs

//! The kinds themselves, one file each: what models a kind uses, what it may
//! stand on, and how thickly it scatters. Adding one is a file here and a
//! member of DecorKind, and nothing else.

pub mod boat;
pub mod car;
pub mod train;
pub mod tree;

/// Every scattered decor kind. A kind's rule is per ground channel: the
/// channel's types it stands on, the channels that must hold nothing, and
/// any other channel is anything. The scatter stays generic, so a later kind
/// (a fence along a road edge, a candle inside a structure, ...) is one more
/// member here and one more file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum DecorKind {
    Tree = 0,
    Boat = 1,
    Car = 2,
    Train = 3,
}

impl DecorKind {
    /// Every member, in the order they are numbered.
    pub const ALL: [DecorKind; 4] = [DecorKind::Tree, DecorKind::Boat, DecorKind::Car, DecorKind::Train];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(value: u8) -> Option<DecorKind> {
        DecorKind::ALL.into_iter().find(|member| *member as u8 == value)
    }
}
