// @pxl/edge/route.spec.ts
//
// GENERATED from spec/spec/route.json
// The address space, as four reserved roots and the branches under them,
// and the two origins they are served from.
//
// The app routes on paths of its own, so a relay tells a deep link from a
// request meant for a server by a root alone — which is why the reservation is
// on the server's side, where it never reaches an address bar.
//
// Every relay reads a root or a branch and never a route name: adding a route
// is a change to the one server that answers it and to nothing else. This file
// is where all of them read it from: the client's driver, the Worker and the
// servers are written from it, and wrangler's and Caddy's configurations take
// their part of it. The kernel names what it opens and never an address.

// The Worker is bundled for the Workers runtime rather than from src/, so it
// cannot import the shell's copy — but it can import this one, which sits
// beside it and is written from the same JSON. It relays by prefix, so the
// prefixes are all it is given.

export const GEO_PREFIX = "/geo/";
export const API_PREFIX = "/api/";
export const TILE_PREFIX = "/tile/";
export const ASSET_PREFIX = "/asset/";
