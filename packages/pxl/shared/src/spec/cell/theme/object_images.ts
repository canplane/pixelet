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

// "cozy" pack — see terrain_images.ts's own cozy import block for
// provenance (pixelet_ms2_texture_manifest.json, downloaded/normalized by
// ./tools/download_texture_pack.ts). Each of the 15 colors gets its own
// distinct felt/wool-style file, never a flat color swatch.
import cozy_wool_white from "./textures/objects/cozy/wool_white.png";
import cozy_wool_grey from "./textures/objects/cozy/wool_grey.png";
import cozy_wool_dark_grey from "./textures/objects/cozy/wool_dark_grey.png";
import cozy_wool_black from "./textures/objects/cozy/wool_black.png";
import cozy_wool_red from "./textures/objects/cozy/wool_red.png";
import cozy_wool_pink from "./textures/objects/cozy/wool_pink.png";
import cozy_wool_brown from "./textures/objects/cozy/wool_brown.png";
import cozy_wool_orange from "./textures/objects/cozy/wool_orange.png";
import cozy_wool_yellow from "./textures/objects/cozy/wool_yellow.png";
import cozy_wool_green from "./textures/objects/cozy/wool_green.png";
import cozy_wool_dark_green from "./textures/objects/cozy/wool_dark_green.png";
import cozy_wool_cyan from "./textures/objects/cozy/wool_cyan.png";
import cozy_wool_blue from "./textures/objects/cozy/wool_blue.png";
import cozy_wool_violet from "./textures/objects/cozy/wool_violet.png";
import cozy_wool_magenta from "./textures/objects/cozy/wool_magenta.png";

/**
 * ch_key -> type name -> texture URL — same two-level shape and same
 * _IMAGE_MAP naming as terrain_images.ts's own GROUND_LAYER_IMAGE_MAP (and
 * ../objects.ts's object_types/object_colors); ../objects.ts's
 * object_types is the numeric identity, this is the matching real
 * material. Kept as a named, non-default pack (see OBJECT_IMAGE_MAP export
 * below) rather than deleted now that cozy is active — still a real,
 * selectable fallback.
 */
export const OBJECT_IMAGE_MAP_SIMPLEPIXEL = {
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

/** cozy's version of the same map — same 15 keys, each pointing at its own
 * distinct cozy wool/felt file (see pixelet_ms2_texture_manifest.json). */
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
export const OBJECT_IMAGE_MAP: typeof OBJECT_IMAGE_MAP_SIMPLEPIXEL = OBJECT_IMAGE_MAP_COZY;
