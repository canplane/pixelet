// pxl-core/domain/setting/store.rs

//! The one settings store: schema.json's defaults with the user's choices
//! over them.
//!
//! Storage keeps only the settings that differ from their defaults. A stored
//! default would be a decision nobody made, and a later change to that
//! default in source would then reach only people whose storage is empty.
//!
//! The storage file (sys/io/file.rs `open_storage`, which checks the slot's
//! format) holds two unsigned LEB128s for each kept setting: the setting's
//! number and its value, a choice by its option's number. schema.json never
//! gives a number out twice, so a kept pair never reads as another setting.
//! A setting that schema.json marks `stored` false lasts for the session
//! alone: it is neither written nor read, so it starts at its default.

pub use crate::spec::schema::{
    AoMode, CameraMode, OrbitObstacleMode, ResMode, Setting, SettingKey,
};
use crate::sys::io::file::{self, StorageRead};
use pxl_global::Global;

static DEV: Global<bool> = Global::new();

/// Whether this is a development build, as the UI's boot reports it
/// (`SETTING_DEV`). Only a development build offers the settings
/// schema.json marks `dev`.
pub fn set_dev(dev: bool) {
    DEV.set(dev);
}

fn dev() -> bool {
    *DEV.get(|| false)
}

/// Whether `val` may be set for `key` in this build. A dev-only setting or
/// option is offered only in a development build, or in a release compiled
/// for measuring (`crate::MEASURING`), whose drive sets them. Storage outlives
/// the build, and a dev value with no row left in the UI to turn it off would
/// otherwise stay in force. Loading drops such a value, and `set_setting`
/// refuses it from the console.
pub fn is_offered(key: SettingKey, val: u32) -> bool {
    let compiled = cfg!(feature = "dev") || crate::MEASURING;
    !(key.dev() || key.dev_option(val)) || (compiled && dev())
}

impl Setting {
    /// Sets `key` to `val` and returns whether it changed. None for a value
    /// the key cannot hold or a setting not offered (`is_offered`). The
    /// caller acts on the change.
    pub fn set_setting(&mut self, key: SettingKey, val: u32) -> Option<bool> {
        if !is_offered(key, val) {
            return None;
        }
        if self.get(key) == val {
            return Some(false);
        }
        if !self.set(key, val) {
            return None;
        }
        if key.is_stored() {
            *UNSAVED.get(|| false) = true;
        }
        Some(true)
    }

    fn storage_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for key in SettingKey::ALL {
            let val = self.get(key);
            if !key.is_stored() || !is_offered(key, val) || val == Setting::DEFAULT.get(key) {
                continue;
            }
            leb(&mut bytes, key as u32);
            leb(&mut bytes, val);
        }
        bytes
    }
}

fn leb(out: &mut Vec<u8>, mut val: u32) {
    while val > 0x7f {
        out.push(val as u8 | 0x80);
        val >>= 7;
    }
    out.push(val as u8);
}

/// Reads one unsigned LEB128 at `at` and moves `at` past it. None when the
/// bytes end inside it or its value does not fit a u32.
fn unleb(bytes: &[u8], at: &mut usize) -> Option<u32> {
    let mut val = 0u64;
    for shift in (0..35).step_by(7) {
        let byte = *bytes.get(*at)?;
        *at += 1;
        val |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return u32::try_from(val).ok();
        }
    }
    None
}

static SETTINGS_STORE: Global<Setting> = Global::new();

/// Whether the store differs from what storage holds. The store is written
/// at most once a frame, so a frame of changes costs the host one file
/// write however many there are.
static UNSAVED: Global<bool> = Global::new();

/// Where the store stands against its storage file, which is opened at the
/// settings' slot (sys/io/file.rs `open_storage`). Until the slot is read, the
/// store holds the defaults and nothing is written over the slot.
#[derive(Clone, Copy, PartialEq)]
enum Load {
    Asked,
    /// The slot was read, whether it held settings or nothing. The store is
    /// written to it from now on.
    Loaded,
    /// The slot was refused (sys/io/file.rs `StorageRead::Refused`). The
    /// store holds the defaults, and does not write over the slot until its
    /// file is set aside.
    Refused,
    /// The slot is there but cannot be read for another reason. The store
    /// holds the defaults and never writes over the slot this session.
    Unreadable,
}

static LOAD: Global<Load> = Global::new();

fn load() -> &'static mut Load {
    LOAD.get(|| Load::Asked)
}

/// Asks for the storage slot. Called at boot.
pub fn settings_open() {
    *load() = Load::Asked;
    file::open_storage();
}

/// Whether the slot was read, whatever it held. From then on the store holds
/// what this session uses, and what waits on the settings may start.
pub fn are_settings_loaded() -> bool {
    *load() != Load::Asked
}

/// Loads the slot once its file arrives, and is true on that frame alone. A
/// setting that is not offered, unknown or invalid stays at its default, and
/// so does every setting when the slot cannot be read.
pub fn settings_load_step() -> bool {
    if are_settings_loaded() {
        return false;
    }
    let Some(file) = file::take_storage() else {
        return false;
    };
    let (s, now) = match file {
        StorageRead::Bytes(bytes) => (read_stored(&bytes), Load::Loaded),
        StorageRead::None => (Setting::DEFAULT, Load::Loaded),
        StorageRead::Refused => {
            file::set_aside(file::Refused::Storage);
            (Setting::DEFAULT, Load::Refused)
        }
        StorageRead::Unreadable => (Setting::DEFAULT, Load::Unreadable),
    };
    SETTINGS_STORE.set(s);
    UNSAVED.set(false);
    *load() = now;
    true
}

/// Called once a refused slot's file is set aside. The store, with what
/// changed meanwhile, is written from the next keep on.
pub fn on_settings_set_aside() {
    if *load() == Load::Refused {
        *load() = Load::Loaded;
    }
}

/// Writes unsaved changes to a slot that was read (`writes`).
pub fn settings_keep_step() {
    if writes() {
        UNSAVED.set(false);
        file::write_storage(&settings().storage_bytes());
    }
}

/// Whether the keep writes the store: there are unsaved changes, and the slot
/// was read. A slot not yet read, refused or unreadable is never written
/// over, and its changes stay in the store alone.
fn writes() -> bool {
    *load() == Load::Loaded && *UNSAVED.get(|| false)
}

/// The stored pairs over the defaults. A pair whose number no setting has is
/// skipped, as is a session's setting and a value its setting cannot hold or
/// this build does not offer. A pair cut short ends the read.
fn read_stored(bytes: &[u8]) -> Setting {
    let mut s = Setting::DEFAULT;
    let mut at = 0;
    while let (Some(number), Some(val)) = (unleb(bytes, &mut at), unleb(bytes, &mut at)) {
        if let Some(key) =
            SettingKey::of(number).filter(|&key| key.is_stored() && is_offered(key, val))
        {
            s.set(key, val);
        }
    }
    s
}

pub fn settings() -> &'static Setting {
    settings_mut()
}

pub fn settings_mut() -> &'static mut Setting {
    SETTINGS_STORE.get(|| Setting::DEFAULT)
}

#[cfg(test)]
#[path = "store.test.rs"]
mod test;
