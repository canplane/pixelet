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
# `deno cache` walks the real local import graph (packages/pxl/shared,
# packages/xpute/core, apps/api/src/**) to know which third-party
# jsr/npm specifiers it even needs, so the source has to be present
# before it can run at all. apps/web's own deno.json (not its src/) is
# still needed just to satisfy the root workspace's own member list.
COPY deno.json deno.lock ./
COPY apps/web/deno.json ./apps/web/deno.json
COPY packages ./packages
COPY apps/api ./apps/api

RUN deno cache apps/api/src/index.ts

EXPOSE 3000

CMD ["deno", "run", "-A", "apps/api/src/index.ts"]
