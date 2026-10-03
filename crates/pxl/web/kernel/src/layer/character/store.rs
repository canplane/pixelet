// pxl-kernel/layer/character/store.rs

use crate::global::Global;

/// Whether the companion's models have loaded: a live, unpersisted signal
/// for the map toast. A fresh figure always starts unloaded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompanionLoadStore {
    pub person_ready: bool,
    pub pet_ready: bool,
}

static STORE: Global<CompanionLoadStore> = Global::new();

pub fn companion_load_store() -> &'static mut CompanionLoadStore {
    STORE.get(CompanionLoadStore::default)
}
