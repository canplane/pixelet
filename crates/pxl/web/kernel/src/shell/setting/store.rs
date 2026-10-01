// pxl-kernel/shell/setting/store.rs

//! The one settings store: what the sheet edits, what the drawer's character
//! picker edits, and what every consumer reads, user-facing and dev alike.
//!
//! Two layers, read as one. schema.json's defaults are the fallback; what the
//! user has set differently lies over them; the store is the two merged
//! (schema.spec.rs `Settings`), so a consumer reads `poi` and never asks
//! which layer it came from.
//!
//! What the browser keeps of them is the shell's (feature/setting/store.ts):
//! it reads storage into a value a setting for WORLD_CREATE, and writes back
//! what the snapshot says whenever it changes. Here a setting is its number
//! and nothing else; its name and its options' are the shell's, which lets
//! the stored form stay one object by name while the module names nothing.

use pxl_spec::bake::ground::{channel_bit, ChannelMask, GroundChannelKey};

use crate::global::Global;
use crate::host::development;
use crate::sched::profiler::log::profiler_follow;
pub use crate::shell::setting::schema_spec::{AoMode, CameraCollisionMode, CameraMode, ResolutionMode, SettingKey, SettingKind, Settings};
use crate::world::chunk::source::chunk_sources_changed;

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
/// value the page cannot reach is a state nothing agrees with, and the
/// shell stores what the store holds, so it leaves storage too.
/// `set_setting` refuses it for the same reason: the row is gone from the
/// sheet, but the command that sets it is not gone from the console.
///
/// A module built without the `dev` feature offers none, whatever the build
/// says of itself: it does not name them (schema.spec.rs `SettingKey::name`),
/// and runs each at its default.
///
/// Called from `settings_load` and after, which means it reads what the
/// shell hands over at WORLD_CREATE: the store is first read after it.
pub fn is_offered(key: SettingKey) -> bool {
    !key.dev() || (cfg!(feature = "dev") && development())
}

/// The ground channels the settings hide, as the paint and `shown_ground`
/// take them: `structures` off hides the structure channel.
pub fn hidden_ground(structures: bool) -> ChannelMask {
    if structures {
        0
    } else {
        channel_bit(GroundChannelKey::STRUCTURE)
    }
}

impl Settings {
    /// Sets `key` to `value` as the command carries it, or does nothing: for
    /// a value the key may not hold, for the value it already has, and for a
    /// setting this build does not offer. The last is the rule
    /// `settings_load` applies to storage,
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
        if key == SettingKey::profiler {
            profiler_follow(value == 1);
        }
    }

    pub fn toggle_setting(&mut self, key: SettingKey) {
        if key.kind() == SettingKind::Toggle {
            self.set_setting(key, 1 - self.get(key));
        }
    }
}

static SETTINGS_STORE: Global<Settings> = Global::new();

/// A setting storage kept nothing for, in what WORLD_CREATE carries.
pub const NONE: u32 = u32::MAX;

/// The settings from what storage kept (WORLD_CREATE): a value a setting in
/// SettingKey's order, NONE for none. What this build does not offer, or a
/// value its setting cannot hold, is left at the default.
pub fn settings_load(values: impl IntoIterator<Item = u32>) {
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
