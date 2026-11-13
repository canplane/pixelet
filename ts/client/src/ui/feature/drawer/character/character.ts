// @pxl/client/ui/feature/drawer/character/character.ts

import type { u32 } from "@xpute/kit/abi/word.ts";

import { SETTING } from "@/ui/shared/setting.ts";
import {
  Gender,
  gender_of,
  Group,
  groups,
} from "@/ui/shared/asset/character.spec.ts";

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

export const GENDERS: readonly Gender[] = [Gender.female, Gender.male];

export const GENDER_LABEL_KEY: Readonly<Record<Gender, string>> = {
  [Gender.female]: "drawer.character.female_label",
  [Gender.male]: "drawer.character.male_label",
};

export function personsOf(gender: Gender): readonly Character[] {
  return PERSONS.filter((v) => groups.person.gender[v.number] === gender);
}

export function genderOf(character: Character): Gender {
  return (
    gender_of(groups.person.gender[character.number] ?? GENDERS[0]) ??
    GENDERS[0]
  );
}

/** The character a setting's number names; the setting's default for a
 * number that names none, or before the core has given one. */
function characterAt<V extends Character>(
  characters: readonly V[],
  number: number | undefined,
  fallback: number,
): V {
  return (
    characters.find((v) => v.number === number) ??
    characters.find((v) => v.number === fallback)!
  );
}

export function personAt(number: number | undefined): Character {
  return characterAt(PERSONS, number, SETTING.person.default);
}

export function petAt(number: number | undefined): Pet {
  return characterAt(PETS, number, SETTING.pet.default);
}
