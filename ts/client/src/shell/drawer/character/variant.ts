// @pxl/client/shell/drawer/character/variant.ts

/** The playable characters from actor.json. Persons are split by gender for the picker. */

import type { u32 } from "@xpute/kit/abi/word.ts";

import { SETTING } from "@/shell/setting/schema.ts";
import { GENDER, Group, groups } from "@/shell/catalog/asset/actor.spec.ts";

export interface CharacterVariant {
  group: u32;
  /** What the kernel holds it as: its row in its group. */
  number: u32;
}

/** A pet, with its name. */
export interface PetVariant extends CharacterVariant {
  label: string;
}

const PERSON_VARIANTS: readonly CharacterVariant[] = groups.person.models.map(
  (_, number) => ({ group: Group.person, number }),
);
export const PET_VARIANTS: readonly PetVariant[] = groups.pet.label.map(
  (label, number) => ({ group: Group.pet, number, label }),
);

/** A person's gender (actor.spec.ts `GENDER`). */
export type Gender = number;
export const GENDERS: readonly Gender[] = [GENDER.female, GENDER.male];

/** Each gender's row title, the locale's key. */
export const GENDER_LABEL_KEY: Readonly<Record<Gender, string>> = {
  [GENDER.female]: "drawer.character.female_label",
  [GENDER.male]: "drawer.character.male_label",
};

export function personVariantsOf(gender: Gender): readonly CharacterVariant[] {
  return PERSON_VARIANTS.filter(
    (v) => groups.person.gender[v.number] === gender,
  );
}

export function genderOf(variant: CharacterVariant): Gender {
  return groups.person.gender[variant.number] ?? GENDERS[0];
}

/** The variant numbered `number`, else the setting's default. */
function numbered<V extends CharacterVariant>(
  variants: readonly V[],
  number: number,
  fallback: number,
): V {
  return (
    variants.find((v) => v.number === number) ??
    variants.find((v) => v.number === fallback)!
  );
}

export function personVariant(number: number): CharacterVariant {
  return numbered(PERSON_VARIANTS, number, SETTING.person.default);
}

export function petVariant(number: number): PetVariant {
  return numbered(PET_VARIANTS, number, SETTING.pet.default);
}
