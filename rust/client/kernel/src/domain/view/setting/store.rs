// pxl-kernel/domain/view/setting/store.rs

//! The one settings store: schema.json's defaults with the user's choices
//! over them. Storage keeps only what differs from the default: a stored
//! default would be a decision nobody made, and changing it in source would
//! then reach only people with empty storage. It keeps them by number, a
//! setting's and its value as it crosses (a choice's option by its number),
//! each pair two unsigned LEB128s, in its kept slot (sys/io/file.rs `Kept`, which
//! checks the slot's format). schema.json gives a number out once, so what
//! was kept never reads as another setting. A setting schema.json marks
//! `kept` false is the session's: neither written nor read, so it starts at
//! its default.

pub use crate::domain::view::setting::schema_spec::{
    AoMode, CameraMode, OracleStart, OrbitObstacleMode, ProxiedMode, ProxyCut, ResolutionMode,
    SettingKey, SettingKind, Settings,
};
use crate::sys::io::file::{self, Kept, KeptFile};
use xpute_guest::global::Global;

static DEVELOPMENT: Global<bool> = Global::new();

/// Development builds offer the settings schema.json marks `dev`, as the
/// shell's boot says (`WORLD_BOOT`).
pub fn set_development(development: bool) {
    DEVELOPMENT.set(development);
}

fn development() -> bool {
    *DEVELOPMENT.get(|| false)
}

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
        if key.kept() {
            *UNKEPT.get(|| false) = true;
        }
        Some(true)
    }

    fn kept(&self) -> Vec<u8> {
        let mut bytes = Vec::new();
        for key in SettingKey::ALL {
            let value = self.get(key);
            if !key.kept() || !is_offered(key, value) || value == Settings::DEFAULT.get(key) {
                continue;
            }
            leb(&mut bytes, key as u32);
            leb(&mut bytes, value);
        }
        bytes
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

/// The store against its kept slot, opened on the storage root as the
/// objects' is (domain/layer/object/repository.rs). Until the slot is read
/// the store holds the defaults and nothing is written over it.
#[derive(Clone, Copy, PartialEq)]
enum Load {
    Asked,
    /// What it held, or nothing: written from now on.
    Loaded,
    /// Refused (sys/io/file.rs `refused`): the defaults, and not written
    /// over until the host sets it aside and empties the slot.
    Refused,
    /// There and not readable here otherwise: the defaults, and never
    /// written over this session.
    Unreadable,
}

static LOAD: Global<Load> = Global::new();

fn load() -> &'static mut Load {
    LOAD.get(|| Load::Asked)
}

/// Asks for the slot, at boot.
pub fn settings_open() {
    *load() = Load::Asked;
    file::open_kept(Kept::Settings);
}

/// The slot was read, whatever it held: from here the store is what this
/// session has, and what waits on it may start.
pub fn settings_loaded() -> bool {
    *load() != Load::Asked
}

/// True the turn the slot is read. What is not offered, unknown or invalid
/// stays at the default, as does every setting where the slot cannot be read.
pub fn settings_load_step() -> bool {
    if settings_loaded() {
        return false;
    }
    let Some(kept) = file::take_kept(Kept::Settings) else {
        return false;
    };
    let (s, now) = match kept {
        KeptFile::Bytes(bytes) => (read_kept(&bytes), Load::Loaded),
        KeptFile::None => (Settings::DEFAULT, Load::Loaded),
        KeptFile::Unreadable if file::refused(Kept::Settings) => (Settings::DEFAULT, Load::Refused),
        KeptFile::Unreadable => (Settings::DEFAULT, Load::Unreadable),
    };
    SETTINGS_STORE.set(s);
    UNKEPT.set(false);
    *load() = now;
    true
}

/// The host set a refused slot's file aside and emptied it: the store, with
/// what changed meanwhile, is written from the next keep on.
pub fn settings_emptied() {
    if *load() == Load::Refused {
        *load() = Load::Loaded;
    }
}

/// Only a slot read is written; a refused or unreadable one holds its changes.
pub fn settings_keep_step() {
    if *load() == Load::Loaded && core::mem::take(UNKEPT.get(|| false)) {
        file::write_kept(Kept::Settings, &settings_store().kept());
    }
}

/// A number no setting has is passed over, as is a session's setting and a
/// value its setting cannot hold, and a pair cut short ends it.
fn read_kept(bytes: &[u8]) -> Settings {
    let mut s = Settings::DEFAULT;
    let mut at = 0;
    while let (Some(number), Some(value)) = (unleb(bytes, &mut at), unleb(bytes, &mut at)) {
        if let Some(key) =
            SettingKey::of(number).filter(|&key| key.kept() && is_offered(key, value))
        {
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
