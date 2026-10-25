# Makefile
#
# Routes only. Each tree is a root of its own: ts/ the TypeScript workspace
# (ts/deno.jsonc), script/ the repository's own scripts, rust/ Cargo's, its
# commands as aliases in rust/.cargo/config.toml, and xpute/ a project of its
# own (xpute/Makefile). Here is a target a tree for each verb, the aggregates
# over them, and what runs across them: the generated code, the dev servers
# and the deploy. Only code is formatted, each tree's by its own formatter;
# the documents and data outside the trees are not. A script that runs alone
# is run by its path, not from here (README.md, Commands). GNU Make 3.81 is enough.
#
#   make -j8 check | test | fmt | lint
#   make check-ts, make test-rust, ...   one tree's
#   make dev | dev-oven | dev-shelf | dev-native | preview | spec | deploy
#   make bake-terrain ARGS=…, make serve-geo, ...   a binary (TOOLS)
#
# With -j the trees run in parallel; xpute's make and cargo share its job
# slots ($(MAKE), and + before cargo).

.PHONY: check check-script check-ts check-rust check-rules check-spec check-build check-xpute \
	test test-ts test-rust test-xpute \
	fmt fmt-script fmt-ts fmt-rust \
	lint lint-script lint-ts \
	spec dev dev-oven dev-shelf dev-native preview preview-oven preview-shelf \
	$(TOOLS) serve-shelf web-oven web-shelf web-preview-oven web-preview-shelf \
	sync-build-number deploy

# `check` is everything, and with `test` what passes before a push to deploy,
# since CI checks nothing (.agents/skills/deploy/SKILL.md). `check-build` is
# what ships, made as a deploy makes it without publishing: the client's
# bundle and the Worker beside it.
check: check-script check-ts check-rust check-rules check-spec check-build check-xpute
check-script:
	deno task --cwd script check
check-ts:
	deno task --cwd ts check
# The module as it ships in both builds, the servers, what runs here alone
# (the tests' code, the bakes, the tools) and both device targets.
check-rust:
	+cd rust && cargo fmt --all --check && cargo lint-guest && cargo lint-guest-published && cargo lint-server && cargo lint-host && cargo lint-ios && cargo lint-android
check-rules:
	+cd rust && cargo check-rules
check-spec:
	+cd rust && cargo spec
check-build:
	deno task --cwd ts/server/edge check:build
check-xpute:
	$(MAKE) -C xpute check

test: test-ts test-rust test-xpute
test-ts:
	deno task --cwd ts test
# pxl-tool holds every model to the committed table, which has been wrong by hand before.
test-rust:
	+cd rust && cargo test-host && cargo test-kernel-published && cargo test-bake-serve
test-xpute:
	$(MAKE) -C xpute test

fmt: fmt-script fmt-ts fmt-rust
fmt-script:
	deno task --cwd script fmt
fmt-ts:
	deno task --cwd ts fmt
fmt-rust:
	cd rust && cargo fmt --all

lint: lint-script lint-ts
lint-script:
	deno task --cwd script lint
lint-ts:
	deno task --cwd ts lint

# The code generated from spec/.
spec:
	+cd rust && cargo spec --write

# Where the tiles come from, a bakery's three: `dev` has them delivered,
# every route the published deployment's, so not for bake work, its tile
# prefix frozen; `dev-oven` bakes each on demand; `dev-shelf` takes each
# kind's set under ./out. The two local ones run the three servers beside the
# client, each its own job, so Ctrl-C stops them all. `preview` is the bundle
# a deploy builds, to measure the client by.
dev: sync-build-number
	deno task --cwd ts/app/web dev
dev-oven: sync-build-number
	$(MAKE) -j4 serve-geo serve-api serve-tile web-oven
dev-shelf: sync-build-number
	$(MAKE) -j4 serve-geo serve-api serve-shelf web-shelf
dev-native:
	deno task --cwd ts/app/native start
preview: sync-build-number
	deno task --cwd ts/app/web preview
preview-oven: sync-build-number
	$(MAKE) -j4 serve-geo serve-api serve-tile web-preview-oven
preview-shelf: sync-build-number
	$(MAKE) -j4 serve-geo serve-api serve-shelf web-preview-shelf

serve-shelf:
	+cd rust && $(CURDIR)/script/with-env.sh cargo serve-tile --shelf ./out
web-oven:
	deno task --cwd ts/app/web dev:oven
web-shelf:
	deno task --cwd ts/app/web dev:shelf
web-preview-oven:
	deno task --cwd ts/app/web preview:oven
web-preview-shelf:
	deno task --cwd ts/app/web preview:shelf

# The binaries that make the repository's data and the dev servers, each
# rust/'s `cargo <name>` (rust/.cargo/config.toml) under the local databases'
# addresses and the servers' numbers (script/with-env.sh). Each works in the
# checkout's root, so ./out and what ARGS names are the root's:
#
#   make bake-terrain ARGS="--shard 01033210 --stage leaf"
#   make bake-geoid ARGS=egm2008-5.pgm
TOOLS := bake-terrain bake-place bake-division bake-earth bake-star bake-geoid \
	download-pack download-moon download-earth download-star ingest-asset render-brand \
	serve-geo serve-api serve-tile
$(TOOLS):
	+cd rust && $(CURDIR)/script/with-env.sh cargo $@ $(ARGS)

# ts/client/.build_number is written by any read of the build log, so it is
# refreshed before vite reads it.
sync-build-number:
	@./script/build-log.py >/dev/null || echo "[build-number] not synced: this build is labeled from the cached number" >&2

# The deployment: the client built and published with the Worker. Does not
# upload what the bucket holds.
deploy:
	deno task --cwd ts/server/edge deploy
