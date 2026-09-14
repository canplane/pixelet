// pxl_core/host.rs
//
// What the kernel asks of the host that is not a command it answers: the
// browser's key-value storage, where the object store keeps its blob. The
// host installs the calls before the first turn; a native test runs on the
// defaults, which keep nothing.

use crate::global::Global;

pub struct Storage {
    pub get: fn(key: &str) -> Option<String>,
    pub set: fn(key: &str, value: &str),
}

fn get_nothing(_: &str) -> Option<String> {
    None
}

fn set_nothing(_: &str, _: &str) {}

static STORAGE: Global<Storage> = Global::new();

/// Installs `localStorage`.
pub fn set_storage(storage: Storage) {
    STORAGE.set(storage);
}

fn storage() -> &'static Storage {
    STORAGE.get(|| Storage { get: get_nothing, set: set_nothing })
}

/// `localStorage.getItem(key)`.
pub fn storage_get(key: &str) -> Option<String> {
    (storage().get)(key)
}

/// `localStorage.setItem(key, value)`.
pub fn storage_set(key: &str, value: &str) {
    (storage().set)(key, value)
}
