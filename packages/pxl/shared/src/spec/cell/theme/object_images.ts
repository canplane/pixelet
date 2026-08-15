// @pxl/shared/spec/cell/theme/object_images.ts

// User-provided (see ./textures/objects/simplepixel/ for the actual
// files). Local import bindings keep the source pack's own wool_* prefix
// (terrain_images.ts's own default_dirt/default_grass/... imports do the
// same with ITS pack's own default_ prefix) — that's just which asset an
// entry happens to be built from, same as terrain_images.ts's own
// BARE: { url: default_grass } pairs a domain key with a differently-named
// asset variable. The exported map's own KEYS are the domain color names
// (white, red, ...), matching object_types.block — "wool" never appears
// there, same reason terrain_types' own channels (land/water/structure/
// transport) never mention "simplepixel" anywhere. Lives here (not
// apps/web, where this used to sit) alongside terrain's own
// terrain_images.ts/textures/terrain/ and ../objects.ts — see
// terrain_images.ts's own comment for why (same-directory-tree relative
// asset imports, not the @pxl/shared/* bare specifier, is what this
// repo's Vite/deno setup actually resolves for non-JS/JSON files). Same
// "simplepixel" pack name as terrain's own texture-pack folder — this is
// the same source pack, just a different subset of its tiles, not a
// distinct pack of its own.
import wool_black from "./textures/objects/simplepixel/wool_black.png";
import wool_blue from "./textures/objects/simplepixel/wool_blue.png";
import wool_brown from "./textures/objects/simplepixel/wool_brown.png";
import wool_cyan from "./textures/objects/simplepixel/wool_cyan.png";
import wool_dark_green from "./textures/objects/simplepixel/wool_dark_green.png";
import wool_dark_grey from "./textures/objects/simplepixel/wool_dark_grey.png";
import wool_green from "./textures/objects/simplepixel/wool_green.png";
import wool_grey from "./textures/objects/simplepixel/wool_grey.png";
import wool_magenta from "./textures/objects/simplepixel/wool_magenta.png";
import wool_orange from "./textures/objects/simplepixel/wool_orange.png";
import wool_pink from "./textures/objects/simplepixel/wool_pink.png";
import wool_red from "./textures/objects/simplepixel/wool_red.png";
import wool_violet from "./textures/objects/simplepixel/wool_violet.png";
import wool_white from "./textures/objects/simplepixel/wool_white.png";
import wool_yellow from "./textures/objects/simplepixel/wool_yellow.png";

/**
 * ch_key -> type name -> texture URL — same two-level shape and same
 * _IMAGE_MAP naming as terrain_images.ts's own GROUND_LAYER_IMAGE_MAP (and
 * ../objects.ts's object_types/object_colors); ../objects.ts's
 * object_types is the numeric identity, this is the matching real
 * material.
 */
export const OBJECT_IMAGE_MAP = {
  block: {
    white: wool_white,
    grey: wool_grey,
    dark_grey: wool_dark_grey,
    black: wool_black,
    red: wool_red,
    pink: wool_pink,
    brown: wool_brown,
    orange: wool_orange,
    yellow: wool_yellow,
    green: wool_green,
    dark_green: wool_dark_green,
    cyan: wool_cyan,
    blue: wool_blue,
    violet: wool_violet,
    magenta: wool_magenta,
  },
} as const;
