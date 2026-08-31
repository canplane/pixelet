// @pxl/web/modules/world/characters/store.ts

import { create } from "zustand";
import { persist } from "zustand/middleware";

import { DEFAULT_PERSON_VARIANT_KEY, DEFAULT_PET_VARIANT_KEY } from "@pxl/shared/assets/characters/variants.ts";

export type CharacterStore = {
  person_variant_key: string;
  set_person_variant_key: (key: string) => void;

  pet_variant_key: string;
  set_pet_variant_key: (key: string) => void;
};

export const useCharacterStore = create<CharacterStore>()(persist((set) => ({
  person_variant_key: DEFAULT_PERSON_VARIANT_KEY,
  set_person_variant_key: (person_variant_key) => set({ person_variant_key }),

  pet_variant_key: DEFAULT_PET_VARIANT_KEY,
  set_pet_variant_key: (pet_variant_key) => set({ pet_variant_key }),
}), { name: "pxl:character_settings" }));
