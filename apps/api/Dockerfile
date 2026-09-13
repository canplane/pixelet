# @pxl/api/Dockerfile
#
# Build context is the repo ROOT (docker build -f apps/api/Dockerfile .),
# not apps/api/ itself — apps/api resolves @pxl/shared/ and @xpute/core/
# through the root deno.json's own workspace import map, not published
# packages, so the image needs the whole workspace tree to resolve those
# imports at all. .dockerignore strips node_modules/.git/out/dist before
# any of this ever reaches the build.
FROM denoland/deno:2.7.11

WORKDIR /app

# No npm-style two-phase "install deps, then copy source" cache split —
# the vite build below walks the real local import graph (packages/pxl/shared,
# packages/xpute/core, apps/api/src/**) to know which third-party
# jsr/npm specifiers it even needs, so the source has to be present
# before it can run at all.
#
# Only what the API's own graph imports is fetched, two things make sure of
# it. apps/web is taken out of the workspace inside the image: its deno.json
# used to be copied in to satisfy the root's member list, and that alone had
# `nodeModulesDir: "auto"` install every npm package the web client declares
# — React, three.js, Tailwind's native binary, lottie — into an image that
# runs none of them; a build failed on one of those downloads ("error
# reading a body from connection" on lottie-web, 2026-09-13). And deno.lock
# is not copied: with a lock present, node_modules is filled from the lock's
# whole npm graph, which carries wrangler and the asset tools' sharp from
# bare `npm:` specifiers the API never runs — 265 MB against 27 MB without
# it, measured. The members' own import ranges pin what is fetched; the API
# in production imports hono, postgres and socket.io and nothing else.
COPY deno.json ./
RUN sed -i '/"apps\/web",/d' deno.json
COPY packages ./packages
COPY apps/api ./apps/api

RUN deno task --cwd apps/api build

EXPOSE 3000

CMD ["deno", "run", "-A", "apps/api/dist/main/server.js"]
