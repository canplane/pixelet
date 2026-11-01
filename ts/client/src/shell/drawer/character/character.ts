// @pxl/client/shell/drawer/character/character.ts

/** The playable characters from actor.json. Persons are split by gender for the picker. */

import type { u32 } from "@xpute/kit/abi/word.ts";

import { SETTING } from "@/shell/setting/schema.ts";
import { GENDER, Group, groups } from "@/shell/catalog/asset/actor.spec.ts";

export interface Character {
  group: u32;
  /** What the kernel holds it as: its row in its group. */
  number: u32;
}

/** A pet, with its name. */
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

/** A person's gender (actor.spec.ts `GENDER`). */
export type Gender = number;
export const GENDERS: readonly Gender[] = [GENDER.female, GENDER.male];

/** Each gender's row title, the locale's key. */
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

/** The character numbered `number`, else the setting's default. */
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
