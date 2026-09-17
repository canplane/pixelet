// @pxl/web/module/world/character/variant.ts

/**
 * The characters a player can be, as variant.json lists them: a group's keys
 * row for row with its models, the colormap the group's pack shares, how tall
 * it stands and which clip is its walk. Each model and skin is fetched by the
 * content key the asset ingest gave it.
 *
 * Split by gender (not one flat PERSON_VARIANTS) so the settings UI can
 * show two separate rows instead of one row a user has to scan by label
 * text to tell apart — display names live in locale files
 * (palette_modal.character.persons.<key>), keyed off `key` below.
 */

import type { f64 } from "@xpute/core/kit/word.ts";

import { INGESTED } from "@pxl/shared/asset/ingested.ts";
import { asset_url } from "@/lib/env/route.ts";
import { DEFAULT_PERSON_VARIANT_KEY as PERSON_KEY, DEFAULT_PET_VARIANT_KEY as PET_KEY, MOVE_CLIP, PERSON, PERSON_GENDER_PREFIXES, PET } from "./variant_spec.ts";

export type CharacterVariant = {
  key: string;
  model_url: string;
  skin_url: string;
  height_u: f64;
  /** Clip name for the catch-up/preview move — Kenney doesn't name this
   * "walk" consistently across packs (person: "sprint", pet: "run"). */
  move_clip: string;
};

/** Where a shipped asset is fetched from, by its path under asset/. */
function shipped_asset_url(path: string): string {
  const row = INGESTED[path];
  if (!row) throw new Error(`${path} is not in @pxl/shared/asset/ingested.ts — run the asset ingest`);
  return asset_url(row.key);
}

type Group = { readonly size: f64; readonly keys: readonly string[]; readonly models: readonly string[]; readonly skin: string };

// Rendering is natively in [u] throughout (U_PER_CELL = 1, terrain/objects
// are all built directly in this unit) — height_u is exactly the on-screen
// height the model is scaled to, and it is the group's `size`: one full cell
// tall for the person, a quarter cell for the pet.
function variants_of(group: Group, move_clip: string): CharacterVariant[] {
  const skin_url = shipped_asset_url(group.skin);
  return group.keys.map((key, i) => ({ key, model_url: shipped_asset_url(group.models[i]), skin_url, height_u: group.size, move_clip }));
}

const PERSON_VARIANTS = variants_of(PERSON, MOVE_CLIP.person);

export const PERSON_VARIANTS_FEMALE: readonly CharacterVariant[] = PERSON_VARIANTS.filter((v) => v.key.startsWith(PERSON_GENDER_PREFIXES.female));
export const PERSON_VARIANTS_MALE: readonly CharacterVariant[] = PERSON_VARIANTS.filter((v) => v.key.startsWith(PERSON_GENDER_PREFIXES.male));
export const PET_VARIANTS: readonly CharacterVariant[] = variants_of(PET, MOVE_CLIP.pet);

export const DEFAULT_PERSON_VARIANT_KEY: string = PERSON_KEY;
export const DEFAULT_PET_VARIANT_KEY: string = PET_KEY;

export function find_person_variant(key: string): CharacterVariant {
  return PERSON_VARIANTS_FEMALE.find((v) => v.key === key) ?? PERSON_VARIANTS_MALE.find((v) => v.key === key) ?? PERSON_VARIANTS_FEMALE[0];
}

export function find_pet_variant(key: string): CharacterVariant {
  return PET_VARIANTS.find((v) => v.key === key) ?? PET_VARIANTS[0];
}
