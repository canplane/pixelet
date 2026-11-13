// pxl-spec/asset/character.spec.rs
//
// GENERATED from spec/spec/asset/character.json (xpute-gen data.rs)
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
    /// The people a player can be, each by its row, the number the settings store
    /// persists. One pack, so one colormap.
    person = 0,
    /// The pets a player's companion can be, each by its row, the number the settings store
    /// persists. One pack, so one colormap.
    pet = 1,
}

impl Group {
    /// Every member, in its source's own order.
    pub const ALL: [Group; 2] = [Group::person, Group::pet];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Group> {
        match val {
            0 => Some(Group::person),
            1 => Some(Group::pet),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Group::person => "person",
            Group::pet => "pet",
        }
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
        pub const models: [u16; 12] = [28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39];
        pub const skin: u16 = 40;
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
        pub const label: &[&str] = &[
            "Beaver",
            "Bee",
            "Bunny",
            "Cat",
            "Caterpillar",
            "Chick",
            "Cow",
            "Crab",
            "Deer",
            "Dog",
            "Elephant",
            "Fish",
            "Fox",
            "Giraffe",
            "Wild Boar",
            "Koala",
            "Lion",
            "Monkey",
            "Panda",
            "Parrot",
            "Penguin",
            "Pig",
            "Polar Bear",
            "Tiger",
        ];
        pub const models: [u16; 24] = [
            41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55, 56, 57, 58, 59, 60, 61, 62,
            63, 64,
        ];
        pub const skin: u16 = 65;
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
    /// Every member, in its source's own order.
    pub const ALL: [Gender; 2] = [Gender::female, Gender::male];

    /// The member `val` is, or none for a number no member has.
    pub const fn of(val: u8) -> Option<Gender> {
        match val {
            0 => Some(Gender::female),
            1 => Some(Gender::male),
            _ => None,
        }
    }

    /// The member's name, as its source spells it.
    pub const fn name(self) -> &'static str {
        match self {
            Gender::female => "female",
            Gender::male => "male",
        }
    }
}
