// pxl-kernel/shell/setting/schema.rs

//! Every setting the app has, declared once in schema.json: its default, its
//! kind, and where the settings sheet shows it. The store (store.rs) reads the
//! defaults from here and the sheet renders rows from the same spec.
//!
//! The values are the fallback layer of a two-layer model, the way an editor
//! keeps a default settings file under the user's own: what the app reads is
//! the defaults with the user's overrides on top, and only the overrides are
//! stored. See store.rs for why that matters.

use crate::host::handheld;
use crate::shell::config::env::APP_ENV;
use crate::shell::setting::schema_spec::SETTINGS as S;

/// Rough per-frame cost, shown as a badge on the row.
pub type SettingCost = &'static str;
/// The sheet's sections, in the order they are shown (schema.json).
pub type SettingSection = &'static str;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingKind {
    Toggle,
    Choice,
    Text,
    Number,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingValue {
    Toggle(bool),
    Text(String),
    Number(u32),
}

/// A setting's value where it lies: a toggle, a text setting's string, or a
/// number.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingRef<'a> {
    Toggle(bool),
    Text(&'a str),
    Number(u32),
}

/// What schema.json declares a setting's default to be: a toggle's, with a
/// handheld device's own where it has one, a text's, or a number's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingDefault {
    Toggle { desktop: bool, handheld: bool },
    Text(&'static str),
    Number(u32),
}

#[derive(Clone, Copy, Debug)]
pub struct Setting {
    pub key: &'static str,
    pub kind: SettingKind,
    pub default: SettingDefault,
    pub section: SettingSection,
    pub cost: Option<SettingCost>,
    pub dev: bool,
    pub options: &'static [&'static str],
}

impl Setting {
    /// The default this device reads.
    pub fn default_value(&self) -> SettingValue {
        match self.default {
            SettingDefault::Toggle { desktop, handheld: h } => SettingValue::Toggle(if handheld() { h } else { desktop }),
            SettingDefault::Text(t) => SettingValue::Text(t.to_string()),
            SettingDefault::Number(n) => SettingValue::Number(n),
        }
    }
}

/// What terrain does to the camera (camera/terrain_gate.rs); schema.json says each.
pub use crate::shell::setting::schema_spec::CAMERA_TERRAIN_MODES;
pub type CameraTerrainMode = &'static str;

pub type SettingKey = &'static str;

const fn toggle(key: &'static str, default: bool, section: &'static str, cost: Option<&'static str>, dev: bool) -> Setting {
    Setting {
        key,
        kind: SettingKind::Toggle,
        default: SettingDefault::Toggle { desktop: default, handheld: default },
        section,
        cost,
        dev,
        options: &[],
    }
}

const fn number(key: &'static str, default: u32, section: &'static str) -> Setting {
    Setting {
        key,
        kind: SettingKind::Number,
        default: SettingDefault::Number(default),
        section,
        cost: None,
        dev: false,
        options: &[],
    }
}

/// Every setting this build has, as schema.json declares it, in declaration
/// order. A `dev` one is in the table under the `dev` feature only: a
/// published module is built without it (Cargo.toml), and a setting that is
/// not in the table is not offered, not stored, and not named anywhere in
/// the module — the store runs it at its default (store.rs `defaults`).
pub const SETTINGS: &[Setting] = &[
    toggle("texture_pack", S::texture_pack::default, S::texture_pack::section, Some(S::texture_pack::cost), false),
    toggle("show_structures", S::show_structures::default, S::show_structures::section, None, false),
    toggle("show_decor", S::show_decor::default, S::show_decor::section, Some(S::show_decor::cost), false),
    #[cfg(feature = "dev")]
    toggle("show_poi", S::show_poi::default, S::show_poi::section, Some(S::show_poi::cost), S::show_poi::dev),
    toggle("lock_camera_tilt", S::lock_camera_tilt::default, S::lock_camera_tilt::section, None, false),
    #[cfg(feature = "dev")]
    Setting {
        key: "camera_terrain",
        kind: SettingKind::Choice,
        default: SettingDefault::Text(S::camera_terrain::default),
        section: S::camera_terrain::section,
        cost: None,
        dev: S::camera_terrain::dev,
        options: S::camera_terrain::options,
    },
    #[cfg(feature = "dev")]
    toggle("ground_snap", S::ground_snap::default, S::ground_snap::section, None, S::ground_snap::dev),
    #[cfg(feature = "dev")]
    toggle(
        "enable_terrain_carving",
        S::enable_terrain_carving::default,
        S::enable_terrain_carving::section,
        None,
        S::enable_terrain_carving::dev,
    ),
    toggle("show_edge_shade", S::show_edge_shade::default, S::show_edge_shade::section, Some(S::show_edge_shade::cost), false),
    toggle("show_rim_light", S::show_rim_light::default, S::show_rim_light::section, Some(S::show_rim_light::cost), false),
    Setting {
        key: "ao",
        kind: SettingKind::Choice,
        default: SettingDefault::Text(S::ao::default),
        section: S::ao::section,
        cost: None,
        dev: false,
        options: S::ao::options,
    },
    #[cfg(feature = "dev")]
    toggle(
        "show_shadow_map",
        S::show_shadow_map::default,
        S::show_shadow_map::section,
        Some(S::show_shadow_map::cost),
        S::show_shadow_map::dev,
    ),
    #[cfg(feature = "dev")]
    Setting {
        default: SettingDefault::Toggle {
            desktop: S::msaa::default,
            handheld: S::msaa::handheld_default,
        },
        ..toggle("msaa", S::msaa::default, S::msaa::section, Some(S::msaa::cost), S::msaa::dev)
    },
    #[cfg(feature = "dev")]
    toggle("pixel_dither", S::pixel_dither::default, S::pixel_dither::section, Some(S::pixel_dither::cost), S::pixel_dither::dev),
    #[cfg(feature = "dev")]
    toggle("show_debug_objects", S::show_debug_objects::default, S::show_debug_objects::section, None, S::show_debug_objects::dev),
    #[cfg(feature = "dev")]
    toggle("show_ledger", S::show_ledger::default, S::show_ledger::section, None, S::show_ledger::dev),
    number("person_variant", S::person_variant::default as u32, S::person_variant::section),
    number("pet_variant", S::pet_variant::default as u32, S::pet_variant::section),
];

/// `SETTING_KEYS`.
#[allow(non_snake_case)]
pub fn SETTING_KEYS() -> impl Iterator<Item = SettingKey> {
    SETTINGS.iter().map(|s| s.key)
}

/// The fallback layer: every setting at the default this device reads.
#[allow(non_snake_case)]
pub fn DEFAULT_SETTINGS() -> Vec<(SettingKey, SettingValue)> {
    SETTINGS.iter().map(|s| (s.key, s.default_value())).collect()
}

/// The settings shown under a section, in declaration order.
pub fn settings_in(section: &str) -> impl Iterator<Item = SettingKey> + '_ {
    SETTINGS.iter().filter(move |s| s.section == section).map(|s| s.key)
}

/// Whether this build offers the setting at all.
///
/// A dev-only setting is not in the sheet outside a development build
/// (feature/setting/schema.ts takes the table without them under
/// `__IS_DEV__`, which is this same test), and what the sheet will not show
/// must not be something the app is running under. Storage outlives the
/// build that wrote it: a value set while the row was there would otherwise
/// stay in force with nothing left to turn it off, and the only way back
/// would be to clear the site's data.
///
/// `load` drops what this says no to rather than carrying it: a value the
/// page cannot reach is a state nothing agrees with. `set_setting` refuses
/// it for the same reason: the row is gone from the sheet, but the action
/// that sets it is not gone from the console.
///
/// Called from `load` and after, which means it reads the environment the
/// shell hands over at WORLD_CREATE — the load order `default_value`
/// already depends on for `handheld`.
pub fn is_offered(key: &str) -> bool {
    let Some(setting) = SETTINGS.iter().find(|s| s.key == key) else { return false };
    !setting.dev || APP_ENV() == "development"
}

/// Whether `value` is something `key` may hold — what a stored override is
/// checked against before it is trusted.
pub fn is_valid_setting(key: &str, value: &SettingValue) -> bool {
    let Some(setting) = SETTINGS.iter().find(|s| s.key == key) else { return false };
    match (setting.kind, value) {
        (SettingKind::Toggle, SettingValue::Toggle(_)) => true,
        (SettingKind::Choice, SettingValue::Text(v)) => setting.options.contains(&v.as_str()),
        (SettingKind::Text, SettingValue::Text(_)) => true,
        (SettingKind::Number, SettingValue::Number(_)) => true,
        _ => false,
    }
}
