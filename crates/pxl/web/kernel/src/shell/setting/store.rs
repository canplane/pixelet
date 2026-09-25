// pxl-kernel/shell/setting/store.rs

//! The one settings store: what the sheet edits, what the drawer's character
//! picker edits, and what every consumer reads, user-facing and dev alike.
//!
//! Two layers, read as one. schema.json's defaults are the fallback; what the
//! user has set differently lies over them; the store is the two merged
//! (schema.spec.rs `Settings`), so a consumer reads `show_poi` and never asks
//! which layer it came from. Only the difference is stored, and a value equal
//! to the default is not: a default that has been written to storage is not a
//! default — it is a decision nobody made, and changing the value in the
//! source would then reach only people with empty storage. A stored value is
//! checked against the schema on load, so a setting that does not exist, or a
//! choice that lacks that option, falls back rather than lingers. A setting
//! this build does not offer goes the same way and for the same reason — a
//! dev-only one has no row outside a development build, so nothing on screen
//! could undo it — and a load that leaves anything out writes the rest back,
//! so what is stored stays what the page can reach.
//!
//! Stored as one object by name, `{"show_poi":true,"ao":"screen"}`: a
//! setting and a choice's option are numbered by their place in schema.json,
//! and a name outlives the order they are listed in.

use core::fmt::Write;

use pxl_spec::bake::ground::{channel_bit, ChannelMask, GroundChannelKey};
use xpute_core::codec::json::{read_json, Quoted};

use crate::global::Global;
use crate::host::{development, storage_get, storage_set};
pub use crate::shell::setting::schema_spec::{AoMode, CameraTerrainMode, SettingKey, SettingKind, Settings};
use crate::world::chunk::source::chunk_sources_changed;

const STORAGE_KEY: &str = "pxl:settings";

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
/// it for the same reason: the row is gone from the sheet, but the command
/// that sets it is not gone from the console.
///
/// A module built without the `dev` feature offers none, whatever the build
/// says of itself: it does not name them (schema.spec.rs `SettingKey::name`),
/// and runs each at its default.
///
/// Called from `load` and after, which means it reads what the shell hands
/// over at WORLD_CREATE: the store is first read after it.
pub fn is_offered(key: SettingKey) -> bool {
    !key.dev() || (cfg!(feature = "dev") && development())
}

/// The ground channels the settings hide, as the paint and `shown_ground`
/// take them: `show_structures` off hides the structure channel.
pub fn hidden_ground(show_structures: bool) -> ChannelMask {
    if show_structures {
        0
    } else {
        channel_bit(GroundChannelKey::STRUCTURE)
    }
}

impl Settings {
    /// Sets `key` to `value` as the command carries it and stores the
    /// difference from the default, or does nothing: for a value the key may
    /// not hold, for the value it already has, and for a setting this build
    /// does not offer. The last is the same rule `load` applies to storage,
    /// kept here for the live path — a dev-only setting has no row outside a
    /// development build, but the command that sets one is reachable from the
    /// console all the same, and what the sheet cannot show it cannot undo.
    pub fn set_setting(&mut self, key: SettingKey, value: u32) {
        if !is_offered(key) || self.get(key) == value || !self.set(key, value) {
            return;
        }
        if key.sources_chunks() {
            chunk_sources_changed();
        }
        save(self);
    }

    pub fn toggle_setting(&mut self, key: SettingKey) {
        if key.kind() == SettingKind::Toggle {
            self.set_setting(key, 1 - self.get(key));
        }
    }
}

/// What differs from the defaults, by name.
fn save(s: &Settings) {
    let defaults = Settings::DEFAULT;
    let mut body = String::from("{");
    for key in SettingKey::ALL {
        let value = s.get(key);
        let Some(name) = key.name().filter(|_| is_offered(key) && value != defaults.get(key)) else {
            continue;
        };
        if body.len() > 1 {
            body.push(',');
        }
        let _ = match key.kind() {
            SettingKind::Toggle => write!(body, "{}:{}", Quoted(name), value == 1),
            SettingKind::Choice => write!(body, "{}:{}", Quoted(name), Quoted(key.option_name(value).unwrap_or_default())),
            SettingKind::Number => write!(body, "{}:{value}", Quoted(name)),
        };
    }
    body.push('}');
    storage_set(STORAGE_KEY, &body);
}

fn load() -> Settings {
    let mut s = Settings::DEFAULT;
    let Some(raw) = storage_get(STORAGE_KEY) else { return s };
    let Some(stored) = read_json(&raw) else {
        save(&s);
        return s;
    };
    let mut left_out = false;
    for (name, value) in stored.fields() {
        let key = name.as_plain().and_then(|name| SettingKey::ALL.into_iter().find(|k| k.name() == Some(name))).filter(|&k| is_offered(k));
        let number = key.and_then(|k| match k.kind() {
            SettingKind::Toggle => value.as_bool().map(u32::from),
            SettingKind::Choice => value.as_str().and_then(|option| k.option_index(option.as_plain()?)),
            SettingKind::Number => value.as_f64().filter(|n| n.fract() == 0.0 && *n >= 0.0).map(|n| n as u32),
        });
        match key.zip(number) {
            Some((key, number)) if number != s.get(key) && s.set(key, number) => {}
            _ => left_out = true,
        }
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

static SETTINGS_STORE: Global<Settings> = Global::new();

pub fn settings_store() -> &'static mut Settings {
    SETTINGS_STORE.get(load)
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
