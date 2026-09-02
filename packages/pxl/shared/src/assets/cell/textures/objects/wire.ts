// @pxl/shared/assets/cell/textures/objects/wire.ts
//
// Bridges the real object-block PNGs in this same textures/ dir to typed
// URLs — not to be confused with spec/cell/objects.ts, which defines the
// numeric object_types/object_colors identity these images just decorate.
// Provenance: tools/manifest.json, downloaded/normalized by
// ./tools/download_texture_pack.ts.
import type { ChannelMap } from "@pxl/shared/spec/cell/type.ts";

import block_white from "./block_white.png";
import block_grey from "./block_grey.png";
import block_dark_grey from "./block_dark_grey.png";
import block_black from "./block_black.png";
import block_red from "./block_red.png";
import block_pink from "./block_pink.png";
import block_brown from "./block_brown.png";
import block_orange from "./block_orange.png";
import block_yellow from "./block_yellow.png";
import block_green from "./block_green.png";
import block_dark_green from "./block_dark_green.png";
import block_cyan from "./block_cyan.png";
import block_blue from "./block_blue.png";
import block_violet from "./block_violet.png";
import block_magenta from "./block_magenta.png";

export default {
  // ch_key -> type name -> texture URL.
  top: {
    block: {
      white: { url: block_white },
      grey: { url: block_grey },
      dark_grey: { url: block_dark_grey },
      black: { url: block_black },
      red: { url: block_red },
      pink: { url: block_pink },
      brown: { url: block_brown },
      orange: { url: block_orange },
      yellow: { url: block_yellow },
      green: { url: block_green },
      dark_green: { url: block_dark_green },
      cyan: { url: block_cyan },
      blue: { url: block_blue },
      violet: { url: block_violet },
      magenta: { url: block_magenta },
    },
  } satisfies ChannelMap,

  side: {} satisfies Partial<ChannelMap>,
};
