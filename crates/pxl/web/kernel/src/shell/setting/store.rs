// pxl-kernel/shell/setting/store.rs

//! The one settings store: what the sheet edits, what the drawer's character
//! picker edits, and what every consumer reads, user-facing and dev alike.
//!
//! Two layers, read as one. schema.json's defaults are the fallback; what the
//! user has set differently lies over them; the store is the two merged
//! (schema.spec.rs `Settings`), so a consumer reads `poi` and never asks
//! which layer it came from.
//!
//! What storage keeps of them is the kernel's too, one file below the storage
//! root (io/file.rs): handed over unread at the boot, as a process is handed
//! its environment, and written by the kernel whenever a setting changes. It is one object by name,
//! `{"poi":true,"ao":"screen"}`: a setting and a choice's option are numbered
//! by their place in schema.json, and a name outlives the order they are
//! listed in. Only what differs from the default is kept: a default written
//! to storage is not a default — it is a decision nobody made, and changing
//! the value in the source would then reach only people with empty storage.

use xpute_core::codec::json::{read_json, JsonKind};

use crate::abi::host::{root, storage, STORAGE_KEYS};
use crate::host::development;
use crate::io::file;
use crate::sched::profiler::log::profiler_follow;
pub use crate::shell::setting::schema_spec::{AoMode, CameraMode, OrbitObstacleMode, ResolutionMode, SettingKey, SettingKind, Settings};
use crate::world::chunk::source::chunk_sources_changed;
use xpute_runtime::global::Global;

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
/// `settings_load` drops what this says no to rather than carrying it: a
/// value the page cannot reach is a state nothing agrees with, and what is
/// kept is written from the store, so it leaves storage too.
/// `set_setting` refuses it for the same reason: the row is gone from the
/// sheet, but the command that sets it is not gone from the console.
///
/// A module built without the `dev` feature offers none, whatever the build
/// says of itself, and runs each at its default.
///
/// Called from `settings_load` and after, which means it reads what
/// WORLD_BOOT said of the build: the store is first read after it.
pub fn is_offered(key: SettingKey) -> bool {
    !key.dev() || (cfg!(feature = "dev") && development())
}

/// `structures` off hides the structure channel.
pub fn hidden_ground(structures: bool) -> ChannelMask {
    if structures {
        0
    } else {
        channel_bit(GroundChannelKey::STRUCTURE)
    }
}

impl Settings {
    /// Does nothing for a value the key may not hold, the current value, or a
    /// setting not offered (see `is_offered`).
    pub fn set_setting(&mut self, key: SettingKey, value: u32) {
        if !is_offered(key) || self.get(key) == value || !self.set(key, value) {
            return;
        }
        if key.sources_chunks() {
            chunk_sources_changed();
        }
        if key == SettingKey::profiler {
            profiler_follow(value == 1);
        }
        self.keep();
    }

    /// Writes what differs from the defaults, by name, to storage.
    fn keep(&self) {
        let mut out = String::from("{");
        for key in SettingKey::ALL {
            let value = self.get(key);
            if !is_offered(key) || value == Settings::DEFAULT.get(key) {
                continue;
            }
            if out.len() > 1 {
                out.push(',');
            }
            let stored = match key.kind() {
                SettingKind::Toggle => (if value == 1 { "true" } else { "false" }).to_string(),
                SettingKind::Choice => format!("\"{}\"", key.options()[value as usize]),
                SettingKind::Index => value.to_string(),
            };
            out.push_str(&format!("\"{}\":{stored}", key.name()));
        }
        out.push('}');
        file::write_whole(root::STORAGE, settings_path(), out.into_bytes());
    }
}

static SETTINGS_STORE: Global<Settings> = Global::new();

fn settings_path() -> &'static str {
    STORAGE_KEYS[storage::SETTINGS as usize]
}

/// The settings from what storage kept, as WORLD_BOOT hands it over. What
/// this build does not offer, a name the schema does not have, or a value
/// its setting cannot hold is left at the default; unreadable storage is
/// none.
pub fn settings_load(kept: &str) {
    let mut s = Settings::DEFAULT;
    if let Some(kept) = read_json(kept).filter(|v| v.kind() == JsonKind::Object) {
        for key in SettingKey::ALL {
            let Some(v) = kept.get(key.name()).filter(|_| is_offered(key)) else { continue };
            let value = match key.kind() {
                SettingKind::Toggle => v.as_bool().map(u32::from),
                SettingKind::Choice => v.as_str().and_then(|o| key.options().iter().position(|n| o.eq_str(n))).map(|i| i as u32),
                SettingKind::Index => v.as_f64().filter(|n| n.fract() == 0.0 && *n >= 0.0 && *n <= u32::MAX as f64).map(|n| n as u32),
            };
            if let Some(value) = value {
                s.set(key, value);
            }
        }
    }
    SETTINGS_STORE.set(s);
    profiler_follow(settings_store().profiler);
}

pub fn settings_store() -> &'static mut Settings {
    SETTINGS_STORE.get(|| Settings::DEFAULT)
}
