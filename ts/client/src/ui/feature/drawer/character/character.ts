// @pxl/client/ui/feature/drawer/character/character.ts

import type { u32 } from "@xpute/kit/abi/word.ts";

import { SETTING } from "@/ui/shared/setting.ts";
import {
  type Gender,
  group,
  GENDER_FEMALE,
  GENDER_MALE,
  GROUP_PERSON,
  GROUP_PET,
} from "@/ui/shared/asset/character.spec.ts";

export interface Character {
  group: u32;
  /** The character's row in its group, which is how the core holds it. */
  number: u32;
}

export interface Pet extends Character {
  label: string;
}

const PERSONS: readonly Character[] = group.person.models.map((_, number) => ({
  group: GROUP_PERSON,
  number,
}));
export const PETS: readonly Pet[] = group.pet.label.map((label, number) => ({
  group: GROUP_PET,
  number,
  label,
}));

export const GENDERS: readonly Gender[] = [GENDER_FEMALE, GENDER_MALE];

export const GENDER_LABEL_KEY: Readonly<Record<Gender, string>> = {
  [GENDER_FEMALE]: "drawer.character.female_label",
  [GENDER_MALE]: "drawer.character.male_label",
};

export function persons_of(gender: Gender): readonly Character[] {
  return PERSONS.filter((v) => group.person.gender[v.number] === gender);
}

export function gender_of(character: Character): Gender {
  const n = group.person.gender[character.number];
  return GENDERS.find((g) => g === n) ?? GENDERS[0];
}

/** The character a setting's number names; the setting's default for a
 * number that names none, or before the core has given one. */
function character_at<V extends Character>(
  characters: readonly V[],
  number: number | undefined,
  fallback: number,
): V {
  return (
    characters.find((v) => v.number === number) ??
    characters.find((v) => v.number === fallback)!
  );
}

export function person_at(number: number | undefined): Character {
  return character_at(PERSONS, number, SETTING.person.default);
}

export function pet_at(number: number | undefined): Pet {
  return character_at(PETS, number, SETTING.pet.default);
}
