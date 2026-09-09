// @pxl/web/lib/env/routes.ts

/**
 * Every same-origin route the app requests, declared once.
 *
 * Source, not environment: every machine and every deployment uses the same
 * paths, and a version bump has to be reviewable. Holding these in a
 * gitignored .env was the reason vite.config.ts grew a parser for a file
 * Vite does not read, and then a rule for when a version applies.
 */

/**
 * A published bake is immutable, so re-baking bumps the version rather than
 * overwriting objects — which invalidates every cache layer at once (CDN
 * edge, browser, any future OPFS cache) with no purge API. A live API bakes
 * on demand and has no version to carry.
 */
export const PUBLISHED_VERSION = {
  terrain: "v2",
  poi: "v2",
  geocode: "v1",
} as const;

export type TileKind = keyof typeof PUBLISHED_VERSION;

/**
 * Everything the server answers, under one root.
 *
 * The app routes on paths of its own (createBrowserRouter), so a relay has
 * to be able to tell a deep link from a request meant for the server. One
 * reserved root does that, and it stays a root rather than a list because
 * what goes under it is not settled — REST resources and a single RPC root
 * are both still open, and a relay that enumerates route names is rewritten
 * when that is decided.
 *
 * The reservation lands on the server's side, not the app's: `/api/...`
 * never reaches an address bar, while giving the app its own prefix would
 * put one in every URL a user sees.
 */
const API_ROOT = "/api";

/** Published bakes: static, versioned, immutable, served from a bucket
 * rather than computed — a routing detail under the same root, not a
 * separate surface. */
const TILE_ROOT = `${API_ROOT}/tiles`;

/** A socket upgrade, not an HTTP route: the API takes it from its own
 * SOCKET_PATH, and the production edge does not proxy it at all. */
const IO_PATH = "/io";

/** `<root>/<kind>[/<version>]`, joined with a page index by the caller. */
export function tile_base(kind: TileKind, published: boolean): string {
  return published ? `${TILE_ROOT}/${kind}/${PUBLISHED_VERSION[kind]}` : `${TILE_ROOT}/${kind}`;
}

/** Forward search resolves against a database, so it is live in every build. */
export const SEARCH_URL = `${API_ROOT}/search`;

/**
 * What a dev server has to forward, and which origin each prefix comes from.
 * A deployment serves all of these itself, so the table exists for the dev
 * server alone — which is why it carries paths and not URLs.
 *
 * Two origins is all the dev server is told, because two is all its .env
 * configures. What is served under a prefix is the origin's business: the
 * proxy forwards each one whole and never learns that terrain, POI and
 * geocode are three different bakes, or which features answer live.
 *
 * Order matters — the tile prefix sits inside the API root, so it has to be
 * tested first. Nothing is rewritten on the way through: every relay and
 * both origins use the same paths the client asks for.
 */
export const DEV_PROXY_ROUTES = [
  { path: TILE_ROOT, origin: "published" },
  { path: API_ROOT, origin: "live" },
  { path: IO_PATH, origin: "live", ws: true },
] as const satisfies readonly {
  path: string;
  origin: "published" | "live";
  ws?: boolean;
}[];

/** What the production edge reads out of R2, and what it forwards to the
 * live API. Mirrored as literals in worker/index.ts, which is bundled for
 * the Workers runtime rather than from src/. */
export const EDGE_TILE_PREFIX = `${TILE_ROOT}/`;
export const EDGE_API_PREFIX = `${API_ROOT}/`;
