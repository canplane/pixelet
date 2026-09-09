// @pxl/shared/bake/published.ts

/**
 * The published version of each bake, read by the bake tools for their
 * output directory and by the client for the path it requests.
 *
 * A published bake is immutable: re-baking bumps the version rather than
 * overwriting objects, which is what invalidates CDN edge and browser
 * caches. Bumping one here is the whole change.
 */
export const PUBLISHED_VERSION = {
  terrain: "v2",
  poi: "v2",
  geocode: "v1",
} as const;

export type BakeKind = keyof typeof PUBLISHED_VERSION;
