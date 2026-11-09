// pxl-spec/asset/character.spec.rs
//
// GENERATED from spec/spec/asset/character.json
//
// The characters: a person or a pet, a model with clips, the
// characters a player can be. Each group's models by their path under
// asset/content/, how tall it stands and which clip is its move. What a
// path is, its number and its extent, is the catalog's (pxl_spec::asset::catalog).

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

/// The groups a character is of, each numbered, and in each its variants,
/// each numbered by its row, row for row with its models, the colormap its pack
/// shares, how tall it stands and which clip is its move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Group {
    person = 0,
    pet = 1,
}

impl Group {
    /// Every member, in the JSON's own order.
    pub const ALL: [Group; 2] = [Group::person, Group::pet];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<Group> {
        Group::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub mod groups {
    /// The people a player can be, each by its row, the number the settings store
    /// persists. One pack, so one colormap.
    pub mod person {
        /// World units tall — one cell.
        pub const size: f64 = 1.0;
        /// Each variant's gender (`GENDER`), row for row with its models.
        pub const gender: [u8; 12] = [0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 1, 1];
        pub const models: [crate::asset::catalog::Slot; 12] = [
            crate::asset::catalog::Slot(28),
            crate::asset::catalog::Slot(29),
            crate::asset::catalog::Slot(30),
            crate::asset::catalog::Slot(31),
            crate::asset::catalog::Slot(32),
            crate::asset::catalog::Slot(33),
            crate::asset::catalog::Slot(34),
            crate::asset::catalog::Slot(35),
            crate::asset::catalog::Slot(36),
            crate::asset::catalog::Slot(37),
            crate::asset::catalog::Slot(38),
            crate::asset::catalog::Slot(39),
        ];
        pub const skin: crate::asset::catalog::Slot = crate::asset::catalog::Slot(40);
        /// The catch-up move's clip, by its index in the pack's animations —
        /// every file of a pack lists them in one order, and Kenney doesn't name this
        /// "walk" consistently: the person's is "sprint", the pet's "run".
        pub const move_clip: u32 = 3;
    }
    /// The pets a player's companion can be, each by its row, the number the settings store
    /// persists. One pack, so one colormap.
    pub mod pet {
        /// World units tall — a quarter cell, the step the object tiers also take.
        pub const size: f64 = 0.25;
        /// Each pet's name, the one every locale shows (an item has one name), row for row with its models.
        pub const label: usize = 24;
        pub const models: [crate::asset::catalog::Slot; 24] = [
            crate::asset::catalog::Slot(41),
            crate::asset::catalog::Slot(42),
            crate::asset::catalog::Slot(43),
            crate::asset::catalog::Slot(44),
            crate::asset::catalog::Slot(45),
            crate::asset::catalog::Slot(46),
            crate::asset::catalog::Slot(47),
            crate::asset::catalog::Slot(48),
            crate::asset::catalog::Slot(49),
            crate::asset::catalog::Slot(50),
            crate::asset::catalog::Slot(51),
            crate::asset::catalog::Slot(52),
            crate::asset::catalog::Slot(53),
            crate::asset::catalog::Slot(54),
            crate::asset::catalog::Slot(55),
            crate::asset::catalog::Slot(56),
            crate::asset::catalog::Slot(57),
            crate::asset::catalog::Slot(58),
            crate::asset::catalog::Slot(59),
            crate::asset::catalog::Slot(60),
            crate::asset::catalog::Slot(61),
            crate::asset::catalog::Slot(62),
            crate::asset::catalog::Slot(63),
            crate::asset::catalog::Slot(64),
        ];
        pub const skin: crate::asset::catalog::Slot = crate::asset::catalog::Slot(65);
        /// The catch-up move's clip, by its index in the pack's animations —
        /// every file of a pack lists them in one order, and Kenney doesn't name this
        /// "walk" consistently: the person's is "sprint", the pet's "run".
        pub const move_clip: u32 = 3;
    }
}

/// The clip a character stands in, "idle", by its index in both packs' animations.
pub const IDLE_CLIP: u32 = 1;

/// A person's gender, which splits the picker into two rows; a person is
/// shown by its place in its row.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum Gender {
    female = 0,
    male = 1,
}

impl Gender {
    /// Every member, in the JSON's own order.
    pub const ALL: [Gender; 2] = [Gender::female, Gender::male];

    /// The member a number names, or none for a number the set does not
    /// give out.
    pub fn of(val: u8) -> Option<Gender> {
        Gender::ALL.into_iter().find(|member| *member as u8 == val)
    }
}

pub mod GENDER {
    pub const female: u8 = 0;
    pub const male: u8 = 1;
}
