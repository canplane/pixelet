// @pxl/web/feature/character/variant.ts

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

import type { f64, u32 } from "@xpute/core/abi/word.ts";

import { INGESTED } from "@/catalog/asset/ingested.spec.ts";
import { asset_url } from "@/config/route.ts";
import { DEFAULT_PERSON_VARIANT_KEY as PERSON_KEY, DEFAULT_PET_VARIANT_KEY as PET_KEY, Group, groups, PERSON_GENDER_PREFIXES } from "./variant.spec.ts";

export interface CharacterVariant {
  /** Its group's number (variant.json's `groups`). */
  group: u32;
  /** Its number in its group: what the kernel holds it as. */
  number: u32;
  /** The key its name is read under (locale). */
  key: string;
  modelUrl: string;
  skinUrl: string;
  height_u: f64;
}

/** Where a shipped asset is fetched from, by its path under asset/. */
function shippedAssetUrl(path: string): string {
  const row = INGESTED[path];
  if (!row) throw new Error(`${path} is not in the asset table — run the asset ingest`);
  return asset_url(row.key);
}

interface GroupSpec {
  readonly size: f64;
  readonly keys: Readonly<Record<string, number>>;
  readonly models: readonly string[];
  readonly skin: string;
}

// Rendering is natively in [u] throughout (U_PER_CELL = 1, terrain/objects
// are all built directly in this unit) — height_u is exactly the on-screen
// height the model is scaled to, and it is the group's `size`: one full cell
// tall for the person, a quarter cell for the pet.
function variantsOf(groupNumber: u32, group: GroupSpec): CharacterVariant[] {
  const skinUrl = shippedAssetUrl(group.skin);
  return Object.entries(group.keys).map(([key, number], i) => ({ group: groupNumber, number, key, modelUrl: shippedAssetUrl(group.models[i]), skinUrl, height_u: group.size }));
}

const PERSON_VARIANTS = variantsOf(Group.person, groups.person);

export const PERSON_VARIANTS_FEMALE: readonly CharacterVariant[] = PERSON_VARIANTS.filter((v) => v.key.startsWith(PERSON_GENDER_PREFIXES.female));
export const PERSON_VARIANTS_MALE: readonly CharacterVariant[] = PERSON_VARIANTS.filter((v) => v.key.startsWith(PERSON_GENDER_PREFIXES.male));
export const PET_VARIANTS: readonly CharacterVariant[] = variantsOf(Group.pet, groups.pet);

export const DEFAULT_PERSON_VARIANT_KEY: string = PERSON_KEY;
export const DEFAULT_PET_VARIANT_KEY: string = PET_KEY;

export function findPersonVariant(key: string): CharacterVariant {
  return PERSON_VARIANTS_FEMALE.find((v) => v.key === key) ?? PERSON_VARIANTS_MALE.find((v) => v.key === key) ?? PERSON_VARIANTS_FEMALE[0];
}

export function findPetVariant(key: string): CharacterVariant {
  return PET_VARIANTS.find((v) => v.key === key) ?? PET_VARIANTS[0];
}
