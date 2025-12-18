import type {
  StyleSpecification,
  VectorSourceSpecification,
} from 'maplibre-gl';

import 'maplibre-gl/dist/maplibre-gl.css';
import localMapStyle from '../assets/open-street-map.json';

/** MapTiler API key (Vite env) */
const MAP_KEY = import.meta.env.VITE_MAPTILER_KEY;

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
    if (!MAP_KEY) {
      throw new Error('VITE_MAPTILER_KEY is not defined');
    }
    return {
      ...(localMapStyle as any),

      // Override vector tile source URL with API key
      sources: {
        openmaptiles: {
          ...localMapStyle.sources.openmaptiles,
          url: localMapStyle.sources.openmaptiles.url.replace('{key}', MAP_KEY),
        } as VectorSourceSpecification,
      },

      // Override glyphs URL with API key
      glyphs: localMapStyle.glyphs.replace('{key}', MAP_KEY),
    };
  },

  /** Hosted MapTiler style URL */
  url(): string {
    if (!MAP_KEY) {
      throw new Error('VITE_MAPTILER_KEY is not defined');
    }
    return `https://api.maptiler.com/maps/019b2b9e-bf3a-75ae-a067-53a775589d8a/style.json?key=${MAP_KEY}`;
  },

  /** Public demo style (for testing without API key) */
  demoUrl(): string {
    return 'https://demotiles.maplibre.org/style.json';
  },
};
