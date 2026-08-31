// @pxl/shared/assets/cell/object_images.ts

// Lives alongside its own ./textures/ (see terrain_images.ts's own
// comment for the full reasoning — same-directory-tree relative imports
// for the PNGs, @pxl/shared/* bare specifier for everything else).

// "cozy" pack — see terrain_images.ts's own cozy import block for
// provenance (pixelet_ms2_texture_manifest.json, downloaded/normalized by
// ./tools/download_texture_pack.ts). Each of the 15 colors gets its own
// distinct felt/wool-style file, never a flat color swatch.
import cozy_wool_white from "./textures/objects/wool_white.png";
import cozy_wool_grey from "./textures/objects/wool_grey.png";
import cozy_wool_dark_grey from "./textures/objects/wool_dark_grey.png";
import cozy_wool_black from "./textures/objects/wool_black.png";
import cozy_wool_red from "./textures/objects/wool_red.png";
import cozy_wool_pink from "./textures/objects/wool_pink.png";
import cozy_wool_brown from "./textures/objects/wool_brown.png";
import cozy_wool_orange from "./textures/objects/wool_orange.png";
import cozy_wool_yellow from "./textures/objects/wool_yellow.png";
import cozy_wool_green from "./textures/objects/wool_green.png";
import cozy_wool_dark_green from "./textures/objects/wool_dark_green.png";
import cozy_wool_cyan from "./textures/objects/wool_cyan.png";
import cozy_wool_blue from "./textures/objects/wool_blue.png";
import cozy_wool_violet from "./textures/objects/wool_violet.png";
import cozy_wool_magenta from "./textures/objects/wool_magenta.png";

/**
 * ch_key -> type name -> texture URL — same two-level shape and same
 * _IMAGE_MAP naming as terrain_images.ts's own GROUND_LAYER_IMAGE_MAP (and
 * ../objects.ts's object_types/object_colors); ../objects.ts's
 * object_types is the numeric identity, this is the matching real
 * material. Each of the 15 colors gets its own distinct cozy wool/felt
 * file (see pixelet_ms2_texture_manifest.json). */
export const OBJECT_IMAGE_MAP_COZY = {
  block: {
    white: cozy_wool_white,
    grey: cozy_wool_grey,
    dark_grey: cozy_wool_dark_grey,
    black: cozy_wool_black,
    red: cozy_wool_red,
    pink: cozy_wool_pink,
    brown: cozy_wool_brown,
    orange: cozy_wool_orange,
    yellow: cozy_wool_yellow,
    green: cozy_wool_green,
    dark_green: cozy_wool_dark_green,
    cyan: cozy_wool_cyan,
    blue: cozy_wool_blue,
    violet: cozy_wool_violet,
    magenta: cozy_wool_magenta,
  },
} as const;

// Active pack — every real consumer (tex.ts, texture_array.ts) imports
// this name only; swap which named pack it points at to change the active
// pack, same pattern terrain_images.ts's own GROUND_LAYER_IMAGE_MAP uses.
export const OBJECT_IMAGE_MAP: typeof OBJECT_IMAGE_MAP_COZY = OBJECT_IMAGE_MAP_COZY;
