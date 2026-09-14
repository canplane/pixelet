// @pxl/web/module/world/character/store.ts

import { create } from "zustand";

/**
 * Whether the in-world companion's person/pet glTF models have finished
 * loading (companion.tsx registers them with the module once parsed) — not
 * persisted, purely a live signal so something outside companion.tsx's own
 * tree (the map toast controller) can tell "is the companion still
 * loading" without reaching into its local state.
 * Starts false on every mount, which is correct: a fresh Walker instance
 * (variant switch remounts it, key={variant.key}) always starts unloaded.
 *
 * Which variant is selected at all (person_variant_key/pet_variant_key)
 * lives in app/setting/schema.ts instead — a real
 * persisted preference, unlike this purely session-scoped loading flag.
 */
export type CompanionLoadStore = {
  person_ready: boolean;
  set_person_ready: (ready: boolean) => void;

  pet_ready: boolean;
  set_pet_ready: (ready: boolean) => void;
};

export const useCompanionLoadStore = create<CompanionLoadStore>((set) => ({
  person_ready: false,
  set_person_ready: (person_ready) => set({ person_ready }),

  pet_ready: false,
  set_pet_ready: (pet_ready) => set({ pet_ready }),
}));
