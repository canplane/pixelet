// pxl-kernel/shell/setting/store.rs

//! The one settings store: what the sheet edits, what the drawer's character
//! picker edits, and what every consumer reads, user-facing and dev alike.
//!
//! Two layers, read as one. `DEFAULT_SETTINGS` (schema.rs) is the fallback;
//! `overrides` is what the user has set differently; the flat fields are the
//! two merged, so a consumer reads `show_poi` and never asks which layer it
//! came from. Only the overrides are stored, and an override equal to the
//! default is dropped rather than kept: a default that has been written to
//! storage is not a default — it is a decision nobody made, and changing
//! the value in the source would then reach only people with empty storage. A
//! stored override is checked against the schema on load, so a setting that
//! does not exist, or a choice that lacks that option, falls back
//! rather than lingers. A setting this build does not offer goes the same
//! way and for the same reason — a dev-only one has no row outside a
//! development build, so nothing on screen could undo it — and a load that
//! leaves anything out writes the rest back, so what is stored stays what
//! the page can reach.
//!
//! Stored as `{"state":{"overrides":{…}},"version":0}`, the shape settings
//! already sit in in a browser's storage.

use xpute_core::codec::json::{read_json, Quoted};

use crate::global::Global;
use crate::host::{handheld, storage_get, storage_set};
use crate::shell::setting::schema::{is_offered, is_valid_setting, SettingKey, SettingRef, SettingValue, DEFAULT_SETTINGS};
use crate::shell::setting::schema_spec::SETTINGS as S;
use crate::world::chunk::node::chunk_sources_changed;

const STORAGE_KEY: &str = "pxl:settings";

/// What the camera does about terrain in its way, as schema.json's
/// `camera_terrain` offers it. The setting crosses as its own text — that is
/// what a stored choice is — and is read as this everywhere past `apply`, so
/// nothing downstream compares strings to decide how the camera behaves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CameraTerrain {
    /// The camera goes where it is pointed, terrain or not.
    Through,
    /// Terrain holds a control back.
    Stop,
    /// Terrain is answered by pitching the view up instead, with every
    /// control still granted.
    #[default]
    Climb,
}

impl CameraTerrain {
    /// The choice `text` names, or the default where it names none: a stored
    /// choice the schema does not offer reads as the default rather than as
    /// a fourth behaviour.
    fn of(text: &str) -> CameraTerrain {
        match text {
            "through" => CameraTerrain::Through,
            "stop" => CameraTerrain::Stop,
            _ => CameraTerrain::Climb,
        }
    }

    /// The name the schema offers it under, which is what a stored choice and
    /// the sheet's own radio both read.
    #[cfg(feature = "dev")]
    fn as_str(self) -> &'static str {
        match self {
            CameraTerrain::Through => "through",
            CameraTerrain::Stop => "stop",
            CameraTerrain::Climb => "climb",
        }
    }
}

/// Where the ambient occlusion comes from, as schema.json's `ao` offers it;
/// read as this past `apply`, as `camera_terrain` is.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AoMode {
    Off,
    /// A chunk shades its own corners (chunk.wgsl's occlusion).
    #[default]
    Corner,
    /// The screen-space pass (pxl-render's gpu/ao.rs).
    Screen,
}

impl AoMode {
    fn of(text: &str) -> AoMode {
        match text {
            "off" => AoMode::Off,
            "screen" => AoMode::Screen,
            _ => AoMode::Corner,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            AoMode::Off => "off",
            AoMode::Corner => "corner",
            AoMode::Screen => "screen",
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsStore {
    pub show_poi: bool,
    pub show_decor: bool,
    pub show_structures: bool,
    pub lock_camera_tilt: bool,
    pub camera_terrain: CameraTerrain,
    pub ground_snap: bool,
    pub enable_terrain_carving: bool,
    pub show_edge_shade: bool,
    pub ao: AoMode,
    pub show_rim_light: bool,
    pub show_shadow_map: bool,
    pub texture_pack: bool,
    pub msaa: bool,
    pub show_debug_objects: bool,
    pub show_ledger: bool,
    pub pixel_dither: bool,
    /// The person's variant, by its number in variant.json's person keys.
    pub person_variant: u8,
    /// The pet's variant, by its number in variant.json's pet keys.
    pub pet_variant: u8,
    /// The user's layer: only what differs from the defaults.
    pub overrides: Vec<(SettingKey, SettingValue)>,
}

impl SettingsStore {
    /// Every setting at schema.json's default for this device, from the
    /// spec's constants rather than from the table: a `dev` setting is not in
    /// the table of a module built without them (schema.rs), and what it does
    /// the module still does, at this default. The test beside this holds the
    /// two to the same values where the table has the setting.
    fn defaults() -> SettingsStore {
        SettingsStore {
            show_poi: S::show_poi::default,
            show_decor: S::show_decor::default,
            show_structures: S::show_structures::default,
            lock_camera_tilt: S::lock_camera_tilt::default,
            camera_terrain: CameraTerrain::of(S::camera_terrain::default),
            ground_snap: S::ground_snap::default,
            enable_terrain_carving: S::enable_terrain_carving::default,
            show_edge_shade: S::show_edge_shade::default,
            ao: AoMode::of(S::ao::default),
            show_rim_light: S::show_rim_light::default,
            show_shadow_map: S::show_shadow_map::default,
            texture_pack: S::texture_pack::default,
            msaa: if handheld() { S::msaa::handheld_default } else { S::msaa::default },
            show_debug_objects: S::show_debug_objects::default,
            show_ledger: S::show_ledger::default,
            pixel_dither: S::pixel_dither::default,
            person_variant: S::person_variant::default,
            pet_variant: S::pet_variant::default,
            overrides: Vec::new(),
        }
    }

    /// `state[key] = value`, for a key the table has. A `dev` key's arm is
    /// under the feature with its row (schema.rs): out of the table, the key
    /// is a string the module has no reason to carry.
    fn write(&mut self, key: &str, value: SettingValue) {
        match (key, value) {
            #[cfg(feature = "dev")]
            ("show_poi", SettingValue::Toggle(v)) => self.show_poi = v,
            ("show_decor", SettingValue::Toggle(v)) => self.show_decor = v,
            ("show_structures", SettingValue::Toggle(v)) => self.show_structures = v,
            ("lock_camera_tilt", SettingValue::Toggle(v)) => self.lock_camera_tilt = v,
            #[cfg(feature = "dev")]
            ("camera_terrain", SettingValue::Text(v)) => self.camera_terrain = CameraTerrain::of(&v),
            #[cfg(feature = "dev")]
            ("ground_snap", SettingValue::Toggle(v)) => self.ground_snap = v,
            #[cfg(feature = "dev")]
            ("enable_terrain_carving", SettingValue::Toggle(v)) => self.enable_terrain_carving = v,
            ("show_edge_shade", SettingValue::Toggle(v)) => self.show_edge_shade = v,
            ("ao", SettingValue::Text(v)) => self.ao = AoMode::of(&v),
            ("show_rim_light", SettingValue::Toggle(v)) => self.show_rim_light = v,
            #[cfg(feature = "dev")]
            ("show_shadow_map", SettingValue::Toggle(v)) => self.show_shadow_map = v,
            ("texture_pack", SettingValue::Toggle(v)) => self.texture_pack = v,
            #[cfg(feature = "dev")]
            ("msaa", SettingValue::Toggle(v)) => self.msaa = v,
            #[cfg(feature = "dev")]
            ("show_debug_objects", SettingValue::Toggle(v)) => self.show_debug_objects = v,
            #[cfg(feature = "dev")]
            ("show_ledger", SettingValue::Toggle(v)) => self.show_ledger = v,
            #[cfg(feature = "dev")]
            ("pixel_dither", SettingValue::Toggle(v)) => self.pixel_dither = v,
            ("person_variant", SettingValue::Number(v)) => self.person_variant = v as u8,
            ("pet_variant", SettingValue::Number(v)) => self.pet_variant = v as u8,
            _ => {}
        }
    }

    /// `state[key]`, where it lies.
    pub fn value(&self, key: &str) -> Option<SettingRef<'_>> {
        Some(match key {
            #[cfg(feature = "dev")]
            "show_poi" => SettingRef::Toggle(self.show_poi),
            "show_decor" => SettingRef::Toggle(self.show_decor),
            "show_structures" => SettingRef::Toggle(self.show_structures),
            "lock_camera_tilt" => SettingRef::Toggle(self.lock_camera_tilt),
            #[cfg(feature = "dev")]
            "camera_terrain" => SettingRef::Text(self.camera_terrain.as_str()),
            #[cfg(feature = "dev")]
            "ground_snap" => SettingRef::Toggle(self.ground_snap),
            #[cfg(feature = "dev")]
            "enable_terrain_carving" => SettingRef::Toggle(self.enable_terrain_carving),
            "show_edge_shade" => SettingRef::Toggle(self.show_edge_shade),
            "ao" => SettingRef::Text(self.ao.as_str()),
            "show_rim_light" => SettingRef::Toggle(self.show_rim_light),
            #[cfg(feature = "dev")]
            "show_shadow_map" => SettingRef::Toggle(self.show_shadow_map),
            "texture_pack" => SettingRef::Toggle(self.texture_pack),
            #[cfg(feature = "dev")]
            "msaa" => SettingRef::Toggle(self.msaa),
            #[cfg(feature = "dev")]
            "show_debug_objects" => SettingRef::Toggle(self.show_debug_objects),
            #[cfg(feature = "dev")]
            "show_ledger" => SettingRef::Toggle(self.show_ledger),
            #[cfg(feature = "dev")]
            "pixel_dither" => SettingRef::Toggle(self.pixel_dither),
            "person_variant" => SettingRef::Number(self.person_variant as u32),
            "pet_variant" => SettingRef::Number(self.pet_variant as u32),
            _ => return None,
        })
    }

    /// `state[key]`.
    pub fn get(&self, key: &str) -> Option<SettingValue> {
        self.value(key).map(|v| match v {
            SettingRef::Toggle(b) => SettingValue::Toggle(b),
            SettingRef::Text(t) => SettingValue::Text(t.to_string()),
            SettingRef::Number(n) => SettingValue::Number(n),
        })
    }

    /// Sets `key` to `value` and stores the difference from the default, or
    /// does nothing: for a value the key may not hold, for the value it
    /// already has, and for a setting this build does not offer. The last is
    /// the same rule `load` applies to storage, kept here for the live path
    /// — a dev-only setting has no row outside a development build, but the
    /// action that sets one is reachable from the console all the same, and
    /// what the sheet cannot show it cannot undo.
    pub fn set_setting(&mut self, key: &str, value: SettingValue) {
        if !is_offered(key) || !is_valid_setting(key, &value) || self.get(key).as_ref() == Some(&value) {
            return;
        }
        let Some((key, default)) = DEFAULT_SETTINGS().into_iter().find(|(k, _)| *k == key) else { return };
        self.overrides.retain(|(k, _)| *k != key);
        if value != default {
            self.overrides.push((key, value.clone()));
        }
        if key == "show_structures" || key == "show_decor" {
            chunk_sources_changed();
        }
        self.write(key, value);
        save(self);
    }

    pub fn toggle_setting(&mut self, key: &str) {
        if let Some(SettingValue::Toggle(v)) = self.get(key) {
            self.set_setting(key, SettingValue::Toggle(!v));
        }
    }
}

fn save(s: &SettingsStore) {
    let body: Vec<String> = s
        .overrides
        .iter()
        .map(|(k, v)| {
            let value = match v {
                SettingValue::Toggle(b) => b.to_string(),
                SettingValue::Text(t) => Quoted(t).to_string(),
                SettingValue::Number(n) => n.to_string(),
            };
            format!("{}:{}", Quoted(k), value)
        })
        .collect();
    storage_set(STORAGE_KEY, &format!("{{\"state\":{{\"overrides\":{{{}}}}},\"version\":0}}", body.join(",")));
}

/// The overrides object of a persisted blob, as pairs of a key and a boolean,
/// string or number value; anything else in it is skipped.
fn parse_overrides(raw: &str) -> Vec<(String, SettingValue)> {
    let Some(overrides) = read_json(raw).and_then(|j| j.get("state")).and_then(|state| state.get("overrides")) else {
        return Vec::new();
    };
    overrides
        .fields()
        .filter_map(|(key, value)| {
            let held = match (value.as_bool(), value.as_str(), value.as_f64()) {
                (Some(on), _, _) => SettingValue::Toggle(on),
                (_, Some(text), _) => SettingValue::Text(text.to_string()),
                (_, _, Some(n)) => SettingValue::Number(n as u32),
                _ => return None,
            };
            Some((key.to_string(), held))
        })
        .collect()
}

fn load() -> SettingsStore {
    let mut s = SettingsStore::defaults();
    let Some(raw) = storage_get(STORAGE_KEY) else { return s };
    let defaults = DEFAULT_SETTINGS();
    let mut left_out = false;
    for (key, value) in parse_overrides(&raw) {
        let Some((key, default)) = defaults.iter().find(|(k, _)| *k == key) else {
            left_out = true;
            continue;
        };
        if !is_offered(key) || !is_valid_setting(key, &value) || value == *default {
            left_out = true;
            continue;
        }
        s.write(key, value.clone());
        s.overrides.push((key, value));
    }
    // Written back wherever something was left out, so that what is stored is
    // what this build can express. A setting it does not offer has no row to
    // undo it by, and a value carried but not applied is a state the page
    // disagrees with; taking it out makes one read enough rather than leaving
    // the storage to be corrected whenever a setting next happens to change.
    // The cost is a development build finding its dev toggles at their
    // defaults after the production one has been opened on the same origin,
    // which is a toggle to flip and not a state to recover.
    if left_out {
        save(&s);
    }
    s
}

static SETTINGS_STORE: Global<SettingsStore> = Global::new();

pub fn settings_store() -> &'static mut SettingsStore {
    SETTINGS_STORE.get(load)
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
