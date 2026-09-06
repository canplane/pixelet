// @pxl/shared/assets/characters/variants.ts

// Lives alongside its own ./kenney_mini_characters/ and
// ./kenney_cube_pets/ (not apps/web, where this used to sit, and not
// ../../spec/, where the domain type-numbering/logic lives) — this file's
// whole job is bridging real model files to typed URLs, so it sits with
// the assets it bridges, same reasoning ../cell/textures/terrain.ts uses
// (see its own comment). Consumers (apps/web) import this file itself via
// the @pxl/shared/* bare specifier; only the asset imports below stay
// relative, since that bare specifier can't be used for non-JS/JSON files.
//
// One import per .glb, not a glob: import.meta.glob's own relative-path
// resolution silently returns no matches once the pattern has to cross out
// of this file's own directory tree the way these paths do (confirmed
// directly — the equivalent glob pattern here left PERSON_VARIANTS/
// PET_VARIANTS empty at runtime with no error, unlike a plain `import`
// statement at the same path depth, which resolves fine). Same reasoning
// cell/textures/terrain.ts and objects.ts already apply to their own dozens of
// PNG imports.
import female_a_url from "./kenney_mini_characters/female-a.glb?url";
import female_b_url from "./kenney_mini_characters/female-b.glb?url";
import female_c_url from "./kenney_mini_characters/female-c.glb?url";
import female_d_url from "./kenney_mini_characters/female-d.glb?url";
import female_e_url from "./kenney_mini_characters/female-e.glb?url";
import female_f_url from "./kenney_mini_characters/female-f.glb?url";
import male_a_url from "./kenney_mini_characters/male-a.glb?url";
import male_b_url from "./kenney_mini_characters/male-b.glb?url";
import male_c_url from "./kenney_mini_characters/male-c.glb?url";
import male_d_url from "./kenney_mini_characters/male-d.glb?url";
import male_e_url from "./kenney_mini_characters/male-e.glb?url";
import male_f_url from "./kenney_mini_characters/male-f.glb?url";
import person_skin_url from "./kenney_mini_characters/colormap.png";

import beaver_url from "./kenney_cube_pets/beaver.glb?url";
import bee_url from "./kenney_cube_pets/bee.glb?url";
import bunny_url from "./kenney_cube_pets/bunny.glb?url";
import cat_url from "./kenney_cube_pets/cat.glb?url";
import caterpillar_url from "./kenney_cube_pets/caterpillar.glb?url";
import chick_url from "./kenney_cube_pets/chick.glb?url";
import cow_url from "./kenney_cube_pets/cow.glb?url";
import crab_url from "./kenney_cube_pets/crab.glb?url";
import deer_url from "./kenney_cube_pets/deer.glb?url";
import dog_url from "./kenney_cube_pets/dog.glb?url";
import elephant_url from "./kenney_cube_pets/elephant.glb?url";
import fish_url from "./kenney_cube_pets/fish.glb?url";
import fox_url from "./kenney_cube_pets/fox.glb?url";
import giraffe_url from "./kenney_cube_pets/giraffe.glb?url";
import hog_url from "./kenney_cube_pets/hog.glb?url";
import koala_url from "./kenney_cube_pets/koala.glb?url";
import lion_url from "./kenney_cube_pets/lion.glb?url";
import monkey_url from "./kenney_cube_pets/monkey.glb?url";
import panda_url from "./kenney_cube_pets/panda.glb?url";
import parrot_url from "./kenney_cube_pets/parrot.glb?url";
import penguin_url from "./kenney_cube_pets/penguin.glb?url";
import pig_url from "./kenney_cube_pets/pig.glb?url";
import polar_url from "./kenney_cube_pets/polar.glb?url";
import tiger_url from "./kenney_cube_pets/tiger.glb?url";
import pet_skin_url from "./kenney_cube_pets/colormap.png";

import type { f64 } from "@xpute/core/kit/word.ts";

export type CharacterVariant = {
  key: string;
  model_url: string;
  skin_url: string;
  height_u: f64;
  /** Clip name for the catch-up/preview move — Kenney doesn't name this
   * "walk" consistently across packs (person: "sprint", pet: "run"). */
  move_clip: string;
};

// Rendering is natively in [u] throughout (U_PER_CELL = 1, terrain/objects
// are all built directly in this unit) — height_u is exactly the on-screen
// height useCharacterModel scales the model to, so these are picked
// directly in [u] rather than run through a meters conversion. One full
// cell tall for the person; a quarter cell for the pet, matching the same
// 0.25 step objects/size.ts's own OBJECT_SIZES already uses.
const PERSON_HEIGHT_U: f64 = 1;
const PET_HEIGHT_U: f64 = 0.25;

// Split by gender (not one flat PERSON_VARIANTS) so the settings UI can show
// two separate rows instead of one row a user has to scan by label text to
// tell apart — display names live in locale files (palette_modal.character.persons.<key>),
// keyed off `key` below, not hardcoded here.
export const PERSON_VARIANTS_FEMALE: readonly CharacterVariant[] = [
  { key: "female-1", model_url: female_a_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "female-2", model_url: female_b_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "female-3", model_url: female_c_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "female-4", model_url: female_d_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "female-5", model_url: female_e_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "female-6", model_url: female_f_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
];

export const PERSON_VARIANTS_MALE: readonly CharacterVariant[] = [
  { key: "male-1", model_url: male_a_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "male-2", model_url: male_b_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "male-3", model_url: male_c_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "male-4", model_url: male_d_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "male-5", model_url: male_e_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
  { key: "male-6", model_url: male_f_url, skin_url: person_skin_url, height_u: PERSON_HEIGHT_U, move_clip: "sprint" },
];

export const PET_VARIANTS: readonly CharacterVariant[] = [
  { key: "beaver", model_url: beaver_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "bee", model_url: bee_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "bunny", model_url: bunny_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "cat", model_url: cat_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "caterpillar", model_url: caterpillar_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "chick", model_url: chick_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "cow", model_url: cow_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "crab", model_url: crab_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "deer", model_url: deer_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "dog", model_url: dog_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "elephant", model_url: elephant_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "fish", model_url: fish_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "fox", model_url: fox_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "giraffe", model_url: giraffe_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "hog", model_url: hog_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "koala", model_url: koala_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "lion", model_url: lion_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "monkey", model_url: monkey_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "panda", model_url: panda_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "parrot", model_url: parrot_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "penguin", model_url: penguin_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "pig", model_url: pig_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "polar", model_url: polar_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
  { key: "tiger", model_url: tiger_url, skin_url: pet_skin_url, height_u: PET_HEIGHT_U, move_clip: "run" },
];

export const DEFAULT_PERSON_VARIANT_KEY: string = "female-2";
export const DEFAULT_PET_VARIANT_KEY: string = "chick";

export function find_person_variant(key: string): CharacterVariant {
  return PERSON_VARIANTS_FEMALE.find((v) => v.key === key) ?? PERSON_VARIANTS_MALE.find((v) => v.key === key) ?? PERSON_VARIANTS_FEMALE[0];
}

export function find_pet_variant(key: string): CharacterVariant {
  return PET_VARIANTS.find((v) => v.key === key) ?? PET_VARIANTS[0];
}
