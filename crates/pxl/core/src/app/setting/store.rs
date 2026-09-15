// pxl_core/app/setting/store.rs

//! The one settings store: what the sheet edits, what the drawer's character
//! picker edits, and what every consumer reads, user-facing and dev alike.
//!
//! Two layers, read as one. `DEFAULT_SETTINGS` (schema.rs) is the fallback;
//! `overrides` is what the user has set differently; the flat fields are the
//! two merged, so a consumer reads `show_poi` and never asks which layer it
//! came from. Only the overrides are stored, and an override equal to the
//! default is dropped rather than kept: a default that has been written to
//! storage is no longer a default — it is a decision nobody made, and changing
//! the value in the source would then reach only people with empty storage. A
//! stored override is checked against the schema on load, so a setting that
//! no longer exists, or a choice that no longer has that option, falls back
//! rather than lingers.
//!
//! Stored under the same key and in the same shape zustand's persist wrote,
//! `{"state":{"overrides":{…}},"version":0}`, so a browser keeps its settings
//! across the move.

use super::schema::{is_valid_setting, SettingKey, SettingRef, SettingValue, DEFAULT_SETTINGS};
use crate::global::Global;
use crate::host::{storage_get, storage_set};

const STORAGE_KEY: &str = "pxl:settings";

#[derive(Clone, Debug, PartialEq)]
pub struct SettingsStore {
    pub show_poi: bool,
    pub show_decor: bool,
    pub show_structures: bool,
    pub lock_camera_tilt: bool,
    pub camera_terrain: String,
    pub ground_snap: bool,
    pub enable_terrain_carving: bool,
    pub show_edge_shade: bool,
    pub show_rim_light: bool,
    pub show_shadow_map: bool,
    pub texture_pack: bool,
    pub msaa: bool,
    pub show_debug_objects: bool,
    pub show_ledger: bool,
    pub pixel_dither: bool,
    pub person_variant_key: String,
    pub pet_variant_key: String,
    /// The user's layer: only what differs from the defaults.
    pub overrides: Vec<(SettingKey, SettingValue)>,
    /// Moves on every change, for the shell's selectors.
    pub revision: u32,
}

impl SettingsStore {
    fn defaults() -> SettingsStore {
        let mut s = SettingsStore {
            show_poi: false,
            show_decor: false,
            show_structures: false,
            lock_camera_tilt: false,
            camera_terrain: String::new(),
            ground_snap: false,
            enable_terrain_carving: false,
            show_edge_shade: false,
            show_rim_light: false,
            show_shadow_map: false,
            texture_pack: false,
            msaa: false,
            show_debug_objects: false,
            show_ledger: false,
            pixel_dither: false,
            person_variant_key: String::new(),
            pet_variant_key: String::new(),
            overrides: Vec::new(),
            revision: 0,
        };
        for (key, value) in DEFAULT_SETTINGS() {
            s.write(key, value);
        }
        s
    }

    fn write(&mut self, key: &str, value: SettingValue) {
        match (key, value) {
            ("show_poi", SettingValue::Toggle(v)) => self.show_poi = v,
            ("show_decor", SettingValue::Toggle(v)) => self.show_decor = v,
            ("show_structures", SettingValue::Toggle(v)) => self.show_structures = v,
            ("lock_camera_tilt", SettingValue::Toggle(v)) => self.lock_camera_tilt = v,
            ("camera_terrain", SettingValue::Text(v)) => self.camera_terrain = v,
            ("ground_snap", SettingValue::Toggle(v)) => self.ground_snap = v,
            ("enable_terrain_carving", SettingValue::Toggle(v)) => self.enable_terrain_carving = v,
            ("show_edge_shade", SettingValue::Toggle(v)) => self.show_edge_shade = v,
            ("show_rim_light", SettingValue::Toggle(v)) => self.show_rim_light = v,
            ("show_shadow_map", SettingValue::Toggle(v)) => self.show_shadow_map = v,
            ("texture_pack", SettingValue::Toggle(v)) => self.texture_pack = v,
            ("msaa", SettingValue::Toggle(v)) => self.msaa = v,
            ("show_debug_objects", SettingValue::Toggle(v)) => self.show_debug_objects = v,
            ("show_ledger", SettingValue::Toggle(v)) => self.show_ledger = v,
            ("pixel_dither", SettingValue::Toggle(v)) => self.pixel_dither = v,
            ("person_variant_key", SettingValue::Text(v)) => self.person_variant_key = v,
            ("pet_variant_key", SettingValue::Text(v)) => self.pet_variant_key = v,
            _ => {}
        }
    }

    /// `state[key]`, where it lies.
    pub fn value(&self, key: &str) -> Option<SettingRef<'_>> {
        Some(match key {
            "show_poi" => SettingRef::Toggle(self.show_poi),
            "show_decor" => SettingRef::Toggle(self.show_decor),
            "show_structures" => SettingRef::Toggle(self.show_structures),
            "lock_camera_tilt" => SettingRef::Toggle(self.lock_camera_tilt),
            "camera_terrain" => SettingRef::Text(&self.camera_terrain),
            "ground_snap" => SettingRef::Toggle(self.ground_snap),
            "enable_terrain_carving" => SettingRef::Toggle(self.enable_terrain_carving),
            "show_edge_shade" => SettingRef::Toggle(self.show_edge_shade),
            "show_rim_light" => SettingRef::Toggle(self.show_rim_light),
            "show_shadow_map" => SettingRef::Toggle(self.show_shadow_map),
            "texture_pack" => SettingRef::Toggle(self.texture_pack),
            "msaa" => SettingRef::Toggle(self.msaa),
            "show_debug_objects" => SettingRef::Toggle(self.show_debug_objects),
            "show_ledger" => SettingRef::Toggle(self.show_ledger),
            "pixel_dither" => SettingRef::Toggle(self.pixel_dither),
            "person_variant_key" => SettingRef::Text(&self.person_variant_key),
            "pet_variant_key" => SettingRef::Text(&self.pet_variant_key),
            _ => return None,
        })
    }

    /// `state[key]`.
    pub fn get(&self, key: &str) -> Option<SettingValue> {
        self.value(key).map(|v| match v {
            SettingRef::Toggle(b) => SettingValue::Toggle(b),
            SettingRef::Text(t) => SettingValue::Text(t.to_string()),
        })
    }

    pub fn set_setting(&mut self, key: &str, value: SettingValue) {
        if !is_valid_setting(key, &value) || self.get(key).as_ref() == Some(&value) {
            return;
        }
        let Some((key, default)) = DEFAULT_SETTINGS().into_iter().find(|(k, _)| *k == key) else { return };
        self.overrides.retain(|(k, _)| *k != key);
        if value != default {
            self.overrides.push((key, value.clone()));
        }
        self.write(key, value);
        self.revision += 1;
        save(self);
    }

    pub fn toggle_setting(&mut self, key: &str) {
        if let Some(SettingValue::Toggle(v)) = self.get(key) {
            self.set_setting(key, SettingValue::Toggle(!v));
        }
    }
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn save(s: &SettingsStore) {
    let body: Vec<String> = s
        .overrides
        .iter()
        .map(|(k, v)| {
            let value = match v {
                SettingValue::Toggle(b) => b.to_string(),
                SettingValue::Text(t) => json_string(t),
            };
            format!("{}:{}", json_string(k), value)
        })
        .collect();
    storage_set(STORAGE_KEY, &format!("{{\"state\":{{\"overrides\":{{{}}}}},\"version\":0}}", body.join(",")));
}

/// The overrides object of a persisted blob, as pairs of a key and a boolean
/// or string value; anything else in it is skipped.
fn parse_overrides(raw: &str) -> Vec<(String, SettingValue)> {
    let mut out = Vec::new();
    let Some(start) = raw.find("\"overrides\"") else { return out };
    let rest = &raw[start + "\"overrides\"".len()..];
    let Some(open) = rest.find('{') else { return out };
    let mut chars = rest[open + 1..].chars().peekable();
    let read_string = |chars: &mut std::iter::Peekable<std::str::Chars>| -> Option<String> {
        let mut s = String::new();
        loop {
            match chars.next()? {
                '"' => return Some(s),
                '\\' => match chars.next()? {
                    'n' => s.push('\n'),
                    't' => s.push('\t'),
                    'u' => {
                        let hex: String = (0..4).filter_map(|_| chars.next()).collect();
                        s.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
                    }
                    c => s.push(c),
                },
                c => s.push(c),
            }
        }
    };
    loop {
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ',') {
            chars.next();
        }
        match chars.next() {
            Some('"') => {}
            _ => return out,
        }
        let Some(key) = read_string(&mut chars) else { return out };
        while chars.peek().is_some_and(|c| c.is_whitespace() || *c == ':') {
            chars.next();
        }
        match chars.peek() {
            Some('"') => {
                chars.next();
                let Some(v) = read_string(&mut chars) else { return out };
                out.push((key, SettingValue::Text(v)));
            }
            Some('t') | Some('f') => {
                let word: String = std::iter::from_fn(|| chars.next_if(|c| c.is_ascii_alphabetic())).collect();
                out.push((key, SettingValue::Toggle(word == "true")));
            }
            _ => {
                // Neither a boolean nor a string: skipped to the next pair.
                while chars.peek().is_some_and(|c| *c != ',' && *c != '}') {
                    chars.next();
                }
            }
        }
    }
}

fn load() -> SettingsStore {
    let mut s = SettingsStore::defaults();
    let Some(raw) = storage_get(STORAGE_KEY) else { return s };
    let defaults = DEFAULT_SETTINGS();
    for (key, value) in parse_overrides(&raw) {
        let Some((key, default)) = defaults.iter().find(|(k, _)| *k == key) else { continue };
        if is_valid_setting(key, &value) && value != *default {
            s.write(key, value.clone());
            s.overrides.push((key, value));
        }
    }
    s
}

static SETTINGS_STORE: Global<SettingsStore> = Global::new();

/// `useSettingsStore.getState()`.
pub fn use_settings_store() -> &'static mut SettingsStore {
    SETTINGS_STORE.get(load)
}
