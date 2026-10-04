// @pxl/web/feature/character/variant.ts

/**
 * The playable characters from variant.json; models and skins are fetched by
 * their ingest content keys. Persons are split by gender for the picker.
 */

import type { f64, u32 } from "@xpute/core/abi/word.ts";

import { INGESTED } from "@/catalog/asset/ingested.spec.ts";
import { assetUrl } from "@/config/route.ts";
import { DEFAULT_PERSON_VARIANT_KEY as PERSON_KEY, DEFAULT_PET_VARIANT_KEY as PET_KEY, Group, groups, PERSON_GENDER_PREFIXES } from "./variant.spec.ts";

export interface CharacterVariant {
  group: u32;
  /** What the kernel holds it as. */
  number: u32;
  key: string;
  modelUrl: string;
  skinUrl: string;
  height_u: f64;
}

function shippedAssetUrl(path: string): string {
  const row = INGESTED[path];
  if (!row) throw new Error(`${path} is not in the asset table — run the asset ingest`);
  return assetUrl(row.key);
}

interface GroupSpec {
  readonly size: f64;
  readonly keys: Readonly<Record<string, number>>;
  readonly models: readonly string[];
  readonly skin: string;
}

// height_u is the on-screen height in world units: the group's `size`.
function variantsOf(groupNumber: u32, group: GroupSpec): CharacterVariant[] {
  const skinUrl = shippedAssetUrl(group.skin);
  return Object.entries(group.keys).map(([key, number], i) => ({ group: groupNumber, number, key, modelUrl: shippedAssetUrl(group.models[i]), skinUrl, height_u: group.size }));
}

const PERSON_VARIANTS = variantsOf(Group.person, groups.person);

export type Gender = keyof typeof PERSON_GENDER_PREFIXES;
export const GENDERS = Object.keys(PERSON_GENDER_PREFIXES) as Gender[];

export function personVariantsOf(gender: Gender): readonly CharacterVariant[] {
  return PERSON_VARIANTS.filter((v) => v.key.startsWith(PERSON_GENDER_PREFIXES[gender]));
}

export function genderOf(key: string): Gender {
  return GENDERS.find((g) => key.startsWith(PERSON_GENDER_PREFIXES[g])) ?? GENDERS[0];
}

export function personVariantNumbered(number: number): CharacterVariant | undefined {
  return PERSON_VARIANTS.find((v) => v.number === number);
}
export const PET_VARIANTS: readonly CharacterVariant[] = variantsOf(Group.pet, groups.pet);

export const DEFAULT_PERSON_VARIANT_KEY: string = PERSON_KEY;
export const DEFAULT_PET_VARIANT_KEY: string = PET_KEY;

export function findPersonVariant(key: string): CharacterVariant {
  return PERSON_VARIANTS.find((v) => v.key === key) ?? personVariantsOf(GENDERS[0])[0];
}

export function findPetVariant(key: string): CharacterVariant {
  return PET_VARIANTS.find((v) => v.key === key) ?? PET_VARIANTS[0];
}
