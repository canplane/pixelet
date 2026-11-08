// @pxl/client/ui/drawer/character/character.ts

import type { u32 } from "@xpute/kit/abi/word.ts";

import { SETTING } from "@/ui/setting/schema.ts";
import { GENDER, Group, groups } from "@/ui/asset/character.spec.ts";

export interface Character {
  group: u32;
  /** The character's row in its group, which is how the core holds it. */
  number: u32;
}

export interface Pet extends Character {
  label: string;
}

const PERSONS: readonly Character[] = groups.person.models.map((_, number) => ({
  group: Group.person,
  number,
}));
export const PETS: readonly Pet[] = groups.pet.label.map((label, number) => ({
  group: Group.pet,
  number,
  label,
}));

export type Gender = number;
export const GENDERS: readonly Gender[] = [GENDER.female, GENDER.male];

export const GENDER_LABEL_KEY: Readonly<Record<Gender, string>> = {
  [GENDER.female]: "drawer.character.female_label",
  [GENDER.male]: "drawer.character.male_label",
};

export function personsOf(gender: Gender): readonly Character[] {
  return PERSONS.filter((v) => groups.person.gender[v.number] === gender);
}

export function genderOf(character: Character): Gender {
  return groups.person.gender[character.number] ?? GENDERS[0];
}

function numbered<V extends Character>(
  characters: readonly V[],
  number: number,
  fallback: number,
): V {
  return (
    characters.find((v) => v.number === number) ??
    characters.find((v) => v.number === fallback)!
  );
}

export function personAt(number: number): Character {
  return numbered(PERSONS, number, SETTING.person.default);
}

export function petAt(number: number): Pet {
  return numbered(PETS, number, SETTING.pet.default);
}
