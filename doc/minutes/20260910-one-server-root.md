# Why everything the server answers sits under one root

**2026-09-10** — routing had drifted into three places that each named
routes for themselves, and fixing one of them exposed the question the split
was avoiding: what, exactly, is allowed to know what the server serves.

> Since split: there are four roots now, `/geo`, `/api`, `/tile` and
> `/asset` (`spec/spec/route.json`), from
> [the server in Rust](./20260918-the-server-in-rust-and-the-line-already-drawn.md)'s
> "The URL roots" and "The frame". What still holds is the rule this
> records: a relay reads a root or a branch and never a route name.

## The bug that started it

`deno task dev:remote` asked the published deployment for
`/tiles/terrain/<page>` and got 404 on every tile, including the shared
`_ocean` fallback. The version token was attached when
`mode === "production"`, and `dev:remote` is the one combination where the
build mode and "reading a published deployment" disagree.

That was a symptom. **"Is this a published bake?" was being re-derived in
three places from three different proxies** — the build mode for the version
token, `VITE_TILE_SOURCE` for the proxy target, and a `"/api/"` string
prefix in `config.ts` for whether a 404 means never-baked. One question with
three answers stays consistent by luck.

## Where the values were kept made the config grow logic

Paths and versions lived in `.env.example` — a file Vite does not read,
chosen because a gitignored `.env` cannot hold a reviewable version bump.
Wanting them reviewed is the tell that they are **source, not environment**:
every machine and every deployment uses the same ones. But putting them
there made `vite.config.ts` parse that file by hand; having parsed it,
compose base URLs; and having composed them, decide when a version applies.

The rule that fell out: a build config may read environment, and must not
derive policy from it.

## The real question: who is allowed to enumerate routes

Three relays sat between the client and the data — Vite's dev proxy, the
Cloudflare Worker, and `wrangler.jsonc`'s `run_worker_first` — and each
named individual routes. `/search` alone appeared in three places, one of
them an exact-match comparison, with a `SEARCH_PATH` var carrying it and a
comment admitting it was kept in sync by hand.

The first instinct was to make the relays dumber by removing `/api`
entirely, so paths would be bare. That is wrong, and the reason is
structural: **the app routes on paths of its own** (`createBrowserRouter`).
A relay cannot tell `/some/page` — a deep link that must fall through to
`index.html` — from a request meant for the server, unless something marks
one of them. Inverting `run_worker_first` to a catch-all does not escape
this; it just moves the marker.

So someone gets a reserved prefix. There were three candidates:

|                             | what it costs                                                                                                         |
| :-------------------------- | :-------------------------------------------------------------------------------------------------------------------- |
| **enumerate at the edge**   | no prefix anywhere, but every new live route is another entry in `run_worker_first` and another literal in the Worker |
| **`/api/*` for the server** | one prefix, invisible to users, relays never learn what is under it                                                   |
| **`/app/*` for the client** | server paths stay bare, but the prefix appears in every URL a user sees                                               |

The reservation lands on the server's side because that is the one that
never reaches an address bar.

## Why a root rather than a tidy list

> Its last paragraph is superseded: the bucket-served tiles have a root of
> their own, `/tile` (see the note at the top).

A list would have removed the duplication too. The root was chosen because
**the shape of the interface is not settled.** A set of REST resources and a
single RPC root — which is what WASM talking to WASM by command ID wants,
and what the April build had — address completely differently, and a relay
that enumerates route names is thrown away by that decision. A root survives
either. HTTP stays a byte transport here; what the bytes mean is the API's
business.

That also decided against splitting `/tiles` and `/api` as two top-level
surfaces. Static-versus-live is a real property of a resource — a published
tile URL is immutable and cacheable forever, a query is not — but it is not
a reason to split the namespace at the root. It is a routing detail one
level down, and the local API had already been serving `/api/tiles/...` all
along; only the edge diverged, and the price of that divergence was the one
per-route rewrite left in the dev proxy.

## What the shape is now

> Superseded. Now `/geo/*` goes to pxl-geo and `/api/*` to pxl-api, both
> relayed by the Worker; `/tile/*` and `/asset/*` are served from the bucket;
> everything else is app assets. `run_worker_first` lists the four roots
> (`apps/web/wrangler.jsonc`), and pxl-tool (`crates/tool/src/bin/spec.rs`)
> writes the roots and branches from `spec/spec/route.json` into
> `apps/client/src/config/route.spec.ts`,
> `apps/web/cloudflare-worker/route.spec.ts` and
> `crates/spec/src/route.spec.rs`. `apps/api`, `src/lib/env/routes.ts`, `/io`
> and `SOCKET_PATH` are gone.

    /api/tiles/<kind>/<version>/<page>   published bakes, bucket-served
    /api/<anything else>                 live API
    everything else                      app assets

`run_worker_first` is one entry. Nothing is rewritten in transit: client,
relay and origin use identical paths. Adding a live route is a change to
`apps/api` and to nothing else. `src/lib/env/routes.ts` declares the routes
for both the client and the dev proxy; the Worker mirrors the two prefixes
as literals because it is bundled for the Workers runtime rather than from
`src/`.

`/io` stays outside: it is a socket upgrade the API takes from its own
`SOCKET_PATH`, and the production edge does not proxy it at all.

## Cost, paid once

Published tile URLs changed, so the CDN and browser caches for the old URLs
go cold once, and the Worker and API have to deploy together — the client
asks for paths the old Worker does not serve, and the reverse. R2 keys are
untouched; only the prefix stripped to reach them changed, so no re-bake and
no re-upload.

## The thing that made this hard to see

The half-migrated state read as a naming inconsistency rather than a
structural one. Six routes were under `/api`, one was not, and the obvious
move — make the seventh match the six — was the wrong direction twice over:
first because the instruction had been to remove `/api`, and then because
removing it turned out to be impossible without giving the prefix to the app
instead. Neither was visible from the diff. It took listing all seven
registrations, the three relays, and what each one keyed on.
