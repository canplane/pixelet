// pxl-kernel/setting/store.rs

//! The one settings store: schema.json's defaults with the user's choices
//! over them. Storage keeps only what differs from the default: a stored
//! default would be a decision nobody made, and changing it in source would
//! then reach only people with empty storage. It keeps them by number, a
//! setting's and its value as it crosses (a choice's option by its number),
//! each pair two unsigned LEB128s after the format's byte, the whole in
//! base64 as storage keeps text. schema.json gives a number out once, so what
//! was kept never reads as another setting. Anything else the file holds,
//! the JSON by name it held before among it, reads as no overrides.

use base64::engine::general_purpose::URL_SAFE_NO_PAD as BASE64;
use base64::Engine;

use crate::io::file::{self, Kept};
use crate::io::host::development;
pub use crate::setting::schema_spec::{AoMode, CameraMode, OrbitObstacleMode, ResolutionMode, SettingKey, SettingKind, Settings};
use xpute_guest::global::Global;

/// The kept form's first byte: pairs by number as this file reads them.
const FORMAT: u8 = 1;

/// A dev-only setting outside a development build is not offered: storage
/// outlives the build, and a value with no row left to turn it off would stay
/// in force. Loading drops it and `set_setting` refuses it from the console.
pub fn is_offered(key: SettingKey, value: u32) -> bool {
    !(key.dev() || key.dev_option(value)) || (cfg!(feature = "dev") && development())
}

impl Settings {
    /// Whether it changed; none for a value the key may not hold or a
    /// setting not offered (see `is_offered`). What a change sets off is its
    /// caller's.
    pub fn set_setting(&mut self, key: SettingKey, value: u32) -> Option<bool> {
        if !is_offered(key, value) {
            return None;
        }
        if self.get(key) == value {
            return Some(false);
        }
        if !self.set(key, value) {
            return None;
        }
        *UNKEPT.get(|| false) = true;
        Some(true)
    }

    fn kept(&self) -> String {
        let mut bytes = vec![FORMAT];
        for key in SettingKey::ALL {
            let value = self.get(key);
            if !is_offered(key, value) || value == Settings::DEFAULT.get(key) {
                continue;
            }
            leb(&mut bytes, key as u32);
            leb(&mut bytes, value);
        }
        BASE64.encode(bytes)
    }
}

/// An unsigned LEB128: seven bits a byte, low first, the top bit set on
/// every byte but the last.
fn leb(out: &mut Vec<u8>, mut value: u32) {
    while value > 0x7f {
        out.push(value as u8 | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// None where the bytes end inside one or it runs past a u32.
fn unleb(bytes: &[u8], at: &mut usize) -> Option<u32> {
    let mut value = 0u64;
    for shift in (0..35).step_by(7) {
        let byte = *bytes.get(*at)?;
        *at += 1;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return u32::try_from(value).ok();
        }
    }
    None
}

static SETTINGS_STORE: Global<Settings> = Global::new();

/// The store differs from what storage holds. Written once a turn, so a turn
/// of changes costs the host stream one file, whatever their number.
static UNKEPT: Global<bool> = Global::new();

pub fn settings_keep_step() {
    if core::mem::take(UNKEPT.get(|| false)) {
        file::write_kept(Kept::Settings, settings_store().kept().into_bytes());
    }
}

/// What is not offered, unknown or invalid stays at the default.
pub fn settings_load(kept: &str) {
    SETTINGS_STORE.set(read_kept(kept));
}

/// A number no setting has is passed over, as is a value its setting cannot
/// hold; what is not base64 or not this format is nothing kept, and a pair
/// cut short ends it.
fn read_kept(kept: &str) -> Settings {
    let mut s = Settings::DEFAULT;
    let bytes = BASE64.decode(kept).unwrap_or_default();
    if bytes.first() != Some(&FORMAT) {
        return s;
    }
    let mut at = 1;
    while let (Some(number), Some(value)) = (unleb(&bytes, &mut at), unleb(&bytes, &mut at)) {
        if let Some(key) = SettingKey::of(number).filter(|&key| is_offered(key, value)) {
            s.set(key, value);
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
