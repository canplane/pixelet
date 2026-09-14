// pxl_core/module/world/character/store.rs

use crate::global::Global;

/// Whether the in-world companion's person/pet models have finished loading
/// (the companion registers them with the kernel once parsed) — not persisted,
/// purely a live signal so something outside the companion (the map toast
/// controller) can tell "is the companion still loading". Starts false on
/// every mount, which is correct: a fresh walker (a variant switch replaces
/// it) always starts unloaded.
///
/// Which variant is selected at all (person_variant_key/pet_variant_key) lives
/// in the settings instead — a real persisted preference, unlike this purely
/// session-scoped loading flag.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct CompanionLoadStore {
    pub person_ready: bool,
    pub pet_ready: bool,
    pub revision: u32,
}

impl CompanionLoadStore {
    pub fn set_person_ready(&mut self, ready: bool) {
        if self.person_ready != ready {
            self.person_ready = ready;
            self.revision += 1;
        }
    }

    pub fn set_pet_ready(&mut self, ready: bool) {
        if self.pet_ready != ready {
            self.pet_ready = ready;
            self.revision += 1;
        }
    }
}

static STORE: Global<CompanionLoadStore> = Global::new();

/// `useCompanionLoadStore.getState()`.
pub fn use_companion_load_store() -> &'static mut CompanionLoadStore {
    STORE.get(CompanionLoadStore::default)
}
