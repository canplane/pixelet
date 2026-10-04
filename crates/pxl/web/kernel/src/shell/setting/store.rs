// pxl-kernel/shell/setting/store.rs

//! The one settings store: schema.json's defaults with the user's choices
//! over them. Storage keeps only what differs from the default, by name: a
//! stored default would be a decision nobody made, and changing it in source
//! would then reach only people with empty storage.

use xpute_core::codec::json::{read_json, JsonKind};

use crate::abi::host::{root, SETTINGS_PATH};
use crate::host::development;
use crate::io::file;
use crate::sched::profiler::log::profiler_follow;
pub use crate::shell::setting::schema_spec::{AoMode, CameraMode, OrbitObstacleMode, ResolutionMode, SettingKey, SettingKind, Settings};
use crate::world::chunk::source::chunk_sources_changed;
use xpute_runtime::global::Global;

/// A dev-only setting outside a development build is not offered: storage
/// outlives the build, and a value with no row left to turn it off would stay
/// in force. Loading drops it and `set_setting` refuses it from the console.
pub fn is_offered(key: SettingKey, value: u32) -> bool {
    !(key.dev() || key.dev_option(value)) || (cfg!(feature = "dev") && development())
}

impl Settings {
    /// Does nothing for a value the key may not hold, the current value, or a
    /// setting not offered (see `is_offered`).
    pub fn set_setting(&mut self, key: SettingKey, value: u32) {
        if !is_offered(key, value) || self.get(key) == value || !self.set(key, value) {
            return;
        }
        if key.sources_chunks() {
            chunk_sources_changed();
        }
        if key == SettingKey::debug_overlay {
            profiler_follow(value == 1);
        }
        self.keep();
    }

    fn keep(&self) {
        file::write_whole(root::STORAGE, settings_path(), self.kept().into_bytes());
    }

    fn kept(&self) -> String {
        let mut out = String::from("{");
        for key in SettingKey::ALL {
            let value = self.get(key);
            if !is_offered(key, value) || value == Settings::DEFAULT.get(key) {
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
        out
    }
}

static SETTINGS_STORE: Global<Settings> = Global::new();

fn settings_path() -> &'static str {
    SETTINGS_PATH
}

/// What is not offered, unknown or invalid stays at the default.
pub fn settings_load(kept: &str) {
    SETTINGS_STORE.set(read_kept(kept));
    profiler_follow(settings_store().debug_overlay);
}

fn read_kept(kept: &str) -> Settings {
    let mut s = Settings::DEFAULT;
    if let Some(kept) = read_json(kept).filter(|v| v.kind() == JsonKind::Object) {
        for key in SettingKey::ALL {
            let Some(v) = kept.get(key.name()) else { continue };
            let value = match key.kind() {
                SettingKind::Toggle => v.as_bool().map(u32::from),
                SettingKind::Choice => v.as_str().and_then(|o| key.options().iter().position(|n| o.eq_str(n))).map(|i| i as u32),
                SettingKind::Index => v.as_f64().filter(|n| n.fract() == 0.0 && *n >= 0.0 && *n <= u32::MAX as f64).map(|n| n as u32),
            };
            if let Some(value) = value.filter(|&value| is_offered(key, value)) {
                s.set(key, value);
            }
        }
    }
    s
}

pub fn settings_store() -> &'static mut Settings {
    SETTINGS_STORE.get(|| Settings::DEFAULT)
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
