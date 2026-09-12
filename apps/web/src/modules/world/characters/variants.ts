// @pxl/web/modules/world/characters/variants.ts

/**
 * The characters a player can be, read off the palette's actor rows
 * (packages/pxl/shared/src/record/shipped.ts): a person is the "person"
 * group, a pet the "pet" group, and each row names its model and its skin
 * by content key. What this file adds is the actor tier's own knowledge —
 * how tall a variant stands and which clip is its walk — which the
 * palette does not carry.
 *
 * Split by gender (not one flat PERSON_VARIANTS) so the settings UI can
 * show two separate rows instead of one row a user has to scan by label
 * text to tell apart — display names live in locale files
 * (palette_modal.character.persons.<key>), keyed off `key` below.
 */

import type { f64 } from "@xpute/core/kit/word.ts";

import { shipped_palette } from "@pxl/shared/record/shipped.ts";
import { asset_url } from "@/lib/env/routes.ts";

export type CharacterVariant = {
  key: string;
  model_url: string;
  skin_url: string;
  height_u: f64;
  /** Clip name for the catch-up/preview move — Kenney doesn't name this
   * "walk" consistently across packs (person: "sprint", pet: "run"). */
  move_clip: string;
};

// Rendering is natively in [u] throughout (U_PER_CELL = 1, terrain/objects
// are all built directly in this unit) — height_u is exactly the on-screen
// height useCharacterModel scales the model to, and it is the palette
// row's `size`: one full cell tall for the person, a quarter cell for the
// pet, matching the 0.25 step objects/size.ts's own OBJECT_SIZES uses.
function variants_of(group: string, move_clip: string): CharacterVariant[] {
  const palette = shipped_palette();
  return palette.rows_of_group(group).map((row) => ({
    key: palette.name_of(row),
    model_url: asset_url(palette.model_key_of(row)),
    skin_url: asset_url(palette.skin_key_of(row)),
    height_u: palette.size[row],
    move_clip,
  }));
}

const PERSON_VARIANTS = variants_of("person", "sprint");

export const PERSON_VARIANTS_FEMALE: readonly CharacterVariant[] = PERSON_VARIANTS.filter((v) => v.key.startsWith("female-"));
export const PERSON_VARIANTS_MALE: readonly CharacterVariant[] = PERSON_VARIANTS.filter((v) => v.key.startsWith("male-"));
export const PET_VARIANTS: readonly CharacterVariant[] = variants_of("pet", "run");

export const DEFAULT_PERSON_VARIANT_KEY: string = "female-2";
export const DEFAULT_PET_VARIANT_KEY: string = "chick";

export function find_person_variant(key: string): CharacterVariant {
  return PERSON_VARIANTS_FEMALE.find((v) => v.key === key) ?? PERSON_VARIANTS_MALE.find((v) => v.key === key) ?? PERSON_VARIANTS_FEMALE[0];
}

export function find_pet_variant(key: string): CharacterVariant {
  return PET_VARIANTS.find((v) => v.key === key) ?? PET_VARIANTS[0];
}
