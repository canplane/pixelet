// @pxl/web/feature/character/variant.ts

/** The playable characters from variant.json. Persons are split by gender for the picker. */

import type { u32 } from "@xpute/core/abi/word.ts";

import { SETTINGS } from "@/feature/setting/schema.ts";
import { Group, groups, PERSON_GENDER_PREFIXES } from "./variant.spec.ts";

export interface CharacterVariant {
  group: u32;
  /** What the kernel holds it as. */
  number: u32;
  key: string;
}

function variantsOf(group: u32, keys: Readonly<Record<string, number>>): readonly CharacterVariant[] {
  return Object.entries(keys).map(([key, number]) => ({ group, number, key }));
}

const PERSON_VARIANTS = variantsOf(Group.person, groups.person.keys);
export const PET_VARIANTS = variantsOf(Group.pet, groups.pet.keys);

export type Gender = keyof typeof PERSON_GENDER_PREFIXES;
export const GENDERS = Object.keys(PERSON_GENDER_PREFIXES) as Gender[];

export function personVariantsOf(gender: Gender): readonly CharacterVariant[] {
  return PERSON_VARIANTS.filter((v) => v.key.startsWith(PERSON_GENDER_PREFIXES[gender]));
}

export function genderOf(variant: CharacterVariant): Gender {
  return GENDERS.find((g) => variant.key.startsWith(PERSON_GENDER_PREFIXES[g])) ?? GENDERS[0];
}

/** The variant numbered `number`, else the setting's default. */
function numbered(variants: readonly CharacterVariant[], number: number, fallback: number): CharacterVariant {
  return variants.find((v) => v.number === number) ?? variants.find((v) => v.number === fallback)!;
}

export function personVariant(number: number): CharacterVariant {
  return numbered(PERSON_VARIANTS, number, SETTINGS.person.default);
}

export function petVariant(number: number): CharacterVariant {
  return numbered(PET_VARIANTS, number, SETTINGS.pet.default);
}
