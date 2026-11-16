// @pxl/client/ui/shared/asset/character.spec.ts
//
// GENERATED from spec/spec/asset/character.json5 (xpute-gen data.rs)
//
// The characters: a person or a pet, a model with clips, the
// characters a player can be. Each group's models by their path under
// asset/content/, how tall it stands and which clip is its move. What a
// path is, its number and its extent, is the catalog's (pxl_spec::asset::catalog).

/** The groups a character is of, each numbered, and in each its variants,
 * each numbered by its row, row for row with its models, the colormap its pack
 * shares, how tall it stands and which clip is its move. */
export const group = {
  /** The people a player can be, each by its row, the number the settings store
   * persists. One pack, so one colormap. */
  person: {
    number: 0 as number,
    /** World units tall — one cell. */
    size: 1 as number,
    /** Each variant's gender (`GENDER`), row for row with its models. */
    gender: [
      0 as number,
      0 as number,
      0 as number,
      0 as number,
      0 as number,
      0 as number,
      1 as number,
      1 as number,
      1 as number,
      1 as number,
      1 as number,
      1 as number,
    ],
    models: [
      27 as number,
      28 as number,
      29 as number,
      30 as number,
      31 as number,
      32 as number,
      33 as number,
      34 as number,
      35 as number,
      36 as number,
      37 as number,
      38 as number,
    ],
    skin: 26 as number,
    /** The catch-up move's clip, by its index in the pack's animations —
     * every file of a pack lists them in one order, and Kenney doesn't name this
     * "walk" consistently: the person's is "sprint", the pet's "run". */
    move_clip: 3 as number,
  },
  /** The pets a player's companion can be, each by its row, the number the settings store
   * persists. One pack, so one colormap. */
  pet: {
    number: 1 as number,
    /** World units tall — a quarter cell, the step the object tiers also take. */
    size: 0.25 as number,
    /** Each pet's name, the one every locale shows (an item has one name), row for row with its models. */
    label: [
      "Beaver",
      "Bee",
      "Bunny",
      "Cat",
      "Caterpillar",
      "Chick",
      "Cow",
      "Crab",
      "Deer",
      "Dog",
      "Elephant",
      "Fish",
      "Fox",
      "Giraffe",
      "Wild Boar",
      "Koala",
      "Lion",
      "Monkey",
      "Panda",
      "Parrot",
      "Penguin",
      "Pig",
      "Polar Bear",
      "Tiger",
    ],
    models: [
      1 as number,
      2 as number,
      3 as number,
      4 as number,
      5 as number,
      6 as number,
      8 as number,
      9 as number,
      10 as number,
      11 as number,
      12 as number,
      13 as number,
      14 as number,
      15 as number,
      16 as number,
      17 as number,
      18 as number,
      19 as number,
      20 as number,
      21 as number,
      22 as number,
      23 as number,
      24 as number,
      25 as number,
    ],
    skin: 7 as number,
    /** The catch-up move's clip, by its index in the pack's animations —
     * every file of a pack lists them in one order, and Kenney doesn't name this
     * "walk" consistently: the person's is "sprint", the pet's "run". */
    move_clip: 3 as number,
  },
} as const;

export type Group = typeof GROUP_PERSON | typeof GROUP_PET;
/** The people a player can be, each by its row, the number the settings store
 * persists. One pack, so one colormap. */
export const GROUP_PERSON = 0;
/** The pets a player's companion can be, each by its row, the number the settings store
 * persists. One pack, so one colormap. */
export const GROUP_PET = 1;

/** The member `val` is, or null for a number no member has. */
export function of_Group(val: number): Group | null {
  return val === GROUP_PERSON || val === GROUP_PET ? val : null;
}

/** The member's name, as its source spells it. */
export function name_Group(val: Group): string {
  switch (val) {
    case GROUP_PERSON:
      return "person";
    case GROUP_PET:
      return "pet";
  }
}

/** The clip a character stands in, "idle", by its index in both packs' animations. */
export const IDLE_CLIP = 1 as number;

/** A person's gender, which splits the picker into two rows; a person is
 * shown by its place in its row. */
export type Gender = typeof GENDER_FEMALE | typeof GENDER_MALE;
export const GENDER_FEMALE = 0;
export const GENDER_MALE = 1;

/** The member `val` is, or null for a number no member has. */
export function of_Gender(val: number): Gender | null {
  return val === GENDER_FEMALE || val === GENDER_MALE ? val : null;
}

/** The member's name, as its source spells it. */
export function name_Gender(val: Gender): string {
  switch (val) {
    case GENDER_FEMALE:
      return "female";
    case GENDER_MALE:
      return "male";
  }
}
