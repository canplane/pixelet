// @pxl/client/shell/catalog/asset/actor.spec.ts
//
// GENERATED from spec/spec/asset/actor.json
//
// The actor kind (id.json): a person or a pet, a model with clips, the
// characters a player can be. Each group's models by their path under
// asset/content/, how tall it stands and which clip is its move. What a
// path is, its number and its extent, is the catalog's (pxl_spec::asset::catalog).

import * as Group from "./actor/group.spec.ts";

/** The groups a character is of, each numbered, and in each its variants by
 * key, row for row with its models, the colormap its pack shares, how tall it
 * stands and which clip is its move. */
export const groups = {
  /** The people a player can be: the key the settings store persists, row for
   * row with the model under asset/. One pack, so one colormap. */
  person: {
    /** World units tall — one cell. */
    size: 1 as number,
    keys: {
      "female-1": 0 as number,
      "female-2": 1 as number,
      "female-3": 2 as number,
      "female-4": 3 as number,
      "female-5": 4 as number,
      "female-6": 5 as number,
      "male-1": 6 as number,
      "male-2": 7 as number,
      "male-3": 8 as number,
      "male-4": 9 as number,
      "male-5": 10 as number,
      "male-6": 11 as number,
    },
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
    /** The catch-up/preview move's clip, by its index in the pack's animations —
     * every file of a pack lists them in one order, and Kenney doesn't name this
     * "walk" consistently: the person's is "sprint", the pet's "run". */
    move_clip: 3 as number,
  },
  /** The pets, the same way. A pet's key is its model's name. */
  pet: {
    /** World units tall — a quarter cell, the step the object tiers also take. */
    size: 0.25 as number,
    keys: {
      beaver: 0 as number,
      bee: 1 as number,
      bunny: 2 as number,
      cat: 3 as number,
      caterpillar: 4 as number,
      chick: 5 as number,
      cow: 6 as number,
      crab: 7 as number,
      deer: 8 as number,
      dog: 9 as number,
      elephant: 10 as number,
      fish: 11 as number,
      fox: 12 as number,
      giraffe: 13 as number,
      hog: 14 as number,
      koala: 15 as number,
      lion: 16 as number,
      monkey: 17 as number,
      panda: 18 as number,
      parrot: 19 as number,
      penguin: 20 as number,
      pig: 21 as number,
      polar: 22 as number,
      tiger: 23 as number,
    },
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
    /** The catch-up/preview move's clip, by its index in the pack's animations —
     * every file of a pack lists them in one order, and Kenney doesn't name this
     * "walk" consistently: the person's is "sprint", the pet's "run". */
    move_clip: 3 as number,
  },
} as const;

export { Group };

/** Split by gender, so the picker shows two rows; a person is shown by
 * its place in its row. */
export const PERSON_GENDER_PREFIX = {
  female: "female-",
  male: "male-",
} as const;
