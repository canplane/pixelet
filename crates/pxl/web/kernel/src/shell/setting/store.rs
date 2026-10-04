// pxl-kernel/shell/setting/store.rs

//! The one settings store: schema.json's defaults with the user's choices
//! over them. Storage keeps only what differs from the default, by name: a
//! stored default would be a decision nobody made, and changing it in source
//! would then reach only people with empty storage.

use pxl_spec::bake::ground::{channel_bit, ChannelMask, GroundChannelKey};

use crate::global::Global;
use crate::host::development;
use crate::sched::profiler::log::profiler_follow;
pub use crate::shell::setting::schema_spec::{AoMode, CameraCollisionMode, CameraMode, ResolutionMode, SettingKey, SettingKind, Settings};
use crate::world::chunk::source::chunk_sources_changed;

/// A dev-only setting outside a development build is not offered: storage
/// outlives the build, and a value with no row left to turn it off would stay
/// in force. Loading drops it and `set_setting` refuses it from the console.
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

    fn keep(&self) {
        file::write_whole(root::STORAGE, settings_path(), self.kept().into_bytes());
    }

    fn kept(&self) -> String {
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
        out
    }

    pub fn toggle_setting(&mut self, key: SettingKey) {
        if key.kind() == SettingKind::Toggle {
            self.set_setting(key, 1 - self.get(key));
        }
    }
}

static SETTINGS_STORE: Global<Settings> = Global::new();

/// A setting storage kept nothing for, in what WORLD_BOOT carries.
pub const NONE: u32 = u32::MAX;

/// What is not offered, unknown or invalid stays at the default.
pub fn settings_load(kept: &str) {
    SETTINGS_STORE.set(read_kept(kept));
    profiler_follow(settings_store().profiler);
}

fn read_kept(kept: &str) -> Settings {
    let mut s = Settings::DEFAULT;
    for (key, value) in SettingKey::ALL.into_iter().zip(values) {
        if value != NONE && is_offered(key) {
            s.set(key, value);
        }
    }
    SETTINGS_STORE.set(s);
    profiler_follow(settings_store().profiler);
}

pub fn settings_store() -> &'static mut Settings {
    SETTINGS_STORE.get(|| Settings::DEFAULT)
}
