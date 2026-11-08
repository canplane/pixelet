// @pxl/client/ui/asset/character.spec.ts
//
// GENERATED from spec/spec/asset/character.json
//
// The character kind (id.json): a person or a pet, a model with clips, the
// characters a player can be. Each group's models by their path under
// asset/content/, how tall it stands and which clip is its move. What a
// path is, its number and its extent, is the catalog's (pxl_spec::asset::catalog).

import * as Group from "./character/group.spec.ts";
import * as GENDER from "./character/gender.spec.ts";

/** The groups a character is of, each numbered, and in each its variants,
 * each numbered by its row, row for row with its models, the colormap its pack
 * shares, how tall it stands and which clip is its move. */
export const groups = {
  /** The people a player can be, each by its row, the number the settings store
   * persists. One pack, so one colormap. */
  person: {
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
  /** The pets, the same way. */
  pet: {
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

export { Group };

/** A person's gender, which splits the picker into two rows; a person is
 * shown by its place in its row. */
export { GENDER };
