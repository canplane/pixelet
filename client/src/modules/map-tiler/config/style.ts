// @pxl/web/modules/map-tiler/config/style.ts

import type { StyleSpecification, VectorSourceSpecification } from "maplibre-gl";

import "maplibre-gl/dist/maplibre-gl.css";
import localMapStyle from "../assets/open-street-map.json" with { type: "json" };

import * as env from "@/lib/env/mod.ts";

/** MapTiler API key (Vite env) */
const MAPTILER_KEY: string | undefined = env.get("VITE_MAPTILER_KEY");

/**
 * Map style resolvers
 *
 * - local(): Use bundled OpenMapTiles style JSON and inject API key
 * - url():   Use hosted MapTiler style URL
 * - demoUrl(): Public demo style (no API key required)
 */
export const mapStyle = {
  /**
   * Build a local style object by injecting MapTiler API key
   * into OpenMapTiles style JSON (sources, glyphs).
   */
  local(): StyleSpecification {
    if (!MAPTILER_KEY) {
      throw new Error("VITE_MAPTILER_KEY is not defined");
    }
    return {
      ...(localMapStyle as any),

      // Override vector tile source URL with API key
      sources: {
        openmaptiles: {
          ...localMapStyle.sources.openmaptiles,
          url: localMapStyle.sources.openmaptiles.url.replace("{key}", MAPTILER_KEY),
        } as VectorSourceSpecification,
      },

      // Override glyphs URL with API key
      glyphs: localMapStyle.glyphs.replace("{key}", MAPTILER_KEY),
    };
  },

  /** Hosted MapTiler style URL */
  url(): string {
    if (!MAPTILER_KEY) {
      throw new Error("VITE_MAPTILER_KEY is not defined");
    }
    return `https://api.maptiler.com/maps/019b2b9e-bf3a-75ae-a067-53a775589d8a/style.json?key=${MAPTILER_KEY}`;
  },

  /** Public demo style (for testing without API key) */
  demoUrl(): string {
    return "https://demotiles.maplibre.org/style.json";
  },
};
