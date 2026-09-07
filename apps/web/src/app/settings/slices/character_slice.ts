// @pxl/web/app/settings/slices/character_slice.ts

import type { StateCreator } from "zustand";

import { DEFAULT_PERSON_VARIANT_KEY, DEFAULT_PET_VARIANT_KEY } from "@pxl/shared/assets/characters/variants.ts";

/** Fields backing modules/world/characters/character_picker.tsx (drawer_modal.tsx's Character tab). */
export type CharacterSettingsSlice = {
  person_variant_key: string;
  set_person_variant_key: (key: string) => void;

  pet_variant_key: string;
  set_pet_variant_key: (key: string) => void;
};

export const create_character_settings_slice: StateCreator<CharacterSettingsSlice, [], [], CharacterSettingsSlice> = (set) => ({
  person_variant_key: DEFAULT_PERSON_VARIANT_KEY,
  set_person_variant_key: (person_variant_key) => set({ person_variant_key }),

  pet_variant_key: DEFAULT_PET_VARIANT_KEY,
  set_pet_variant_key: (pet_variant_key) => set({ pet_variant_key }),
});
