# Makefile
#
# Routes only. Each tree is a root of its own: ts/ the TypeScript workspace
# (ts/pnpm-workspace.yaml), rust/ Cargo's, its commands as aliases in
# rust/.cargo/config.toml, and xpute/ a project of its own.
# script/ holds the repository's own scripts, each in the language its work
# takes and run by its path. Here is a target a tree for each verb, the aggregates
# over them, and what runs across them: the generated code, the dev servers
# and the deploy. Only code is formatted, each tree's by its own formatter;
# the documents and data outside the trees are not. A script that runs alone
# is run by its path, not from here (README.md, Commands). GNU Make 3.81 is enough.
#
#   make install | -j8 check | test | fmt
#   make check-ts, make test-rust, ...   one tree's
#   make check-build | check-xpute | test-xpute | test-gpu   what a deploy adds
#   make dev | dev-oven | dev-shelf | dev-native | preview | spec | deploy
#   make bake-terrain ARGS=…, make serve-geo, ...   a binary (TOOLS)
#
# With -j the trees run in parallel; xpute's make and cargo share its job
# slots ($(MAKE), and + before cargo).

.PHONY: install install-ts install-xpute install-rust \
	check check-ts check-rust check-rules check-spec check-build check-xpute \
	test test-ts test-rust test-xpute test-gpu \
	fmt fmt-ts fmt-rust \
	spec dev dev-oven dev-shelf dev-native preview preview-oven preview-shelf \
	serve-tile watch-wasm web web-preview \
	sync-build-number deploy

# Each tree's packages, by its own manager: ts/'s here, xpute's by its make,
# and the cargo tools the tests run under.
install: install-ts install-xpute install-rust
install-ts:
	pnpm --dir ts install
install-xpute:
	pnpm --dir xpute/ts install
install-rust:
	cargo install cargo-nextest --locked

# `check` and `test` hold pixelet's code. What a deploy adds before its push,
# since CI checks nothing (.agents/skills/deploy/SKILL.md): `check-build`, the
# client's bundle as a deploy builds it (the Worker is bundled by the deploy
# itself, in CI, which runs wrangler); xpute's own checks and tests, a project
# of its own; and `test-gpu`, the renderer's passes on this machine's GPU.
check: check-ts check-rust check-rules check-spec
check-ts:
	pnpm --dir ts check
# The module as it ships in both builds, the servers, what runs here alone
# (the tests' code, the bakes, the tools) and both device targets.
check-rust:
	+cd rust && cargo fmt --check && cargo lint-core && cargo lint-core-published && cargo lint-server && cargo lint-host && cargo lint-ios && $(CURDIR)/script/env.sh cargo lint-android
check-rules:
	+cd rust && cargo check-rules
check-spec:
	+cd rust && cargo spec
check-build:
	pnpm --dir ts/client build:web
check-xpute:
	+cd xpute/rust && cargo fmt --check && cargo spec && cargo clippy --workspace --all-targets -- -D warnings
	pnpm --dir xpute/ts check
	$(MAKE) -C xpute/cpp check

test: test-ts test-rust
test-ts:
	pnpm --dir ts test
test-rust:
	@cargo nextest --version >/dev/null 2>&1 || { echo "pxl-core's unit tests run under cargo-nextest, which make install brings" >&2; exit 1; }
	+cd rust && cargo test-host && cargo test-core && cargo test-frame && cargo test-render
test-xpute:
	+cd xpute/rust && cargo test --workspace && cargo core && cargo core-wasm
	pnpm --dir xpute/ts test
	$(MAKE) -C xpute/cpp test
	pnpm --dir xpute/ts test:conformance
	pnpm --dir xpute/ts test:example
test-gpu:
	+cd rust && cargo test-gpu

fmt: fmt-ts fmt-rust
fmt-ts:
	pnpm --dir ts fmt
fmt-rust:
	cd rust && cargo fmt

# The code generated from spec/.
spec:
	+cd rust && cargo spec --write
	+cd xpute/rust && cargo spec --write

# Where the tiles come from (SOURCE), a bakery's three: `dev` has them delivered,
# every route the published deployment's, so not for bake work, its tile
# prefix frozen; `dev-oven` bakes each on demand; `dev-shelf` takes each
# kind's set under ./out. The two local ones run the three servers beside the
# client, each its own job, so Ctrl-C stops them all. Each dev target runs
# the module's watcher beside the client (ts/client/script/wasm-watch.py): a
# change under rust/ rebuilds public/core.wasm and the page is reloaded by
# hand. `preview` is the bundle a deploy builds, to measure the client by.
dev: sync-build-number
	$(MAKE) -j2 watch-wasm web
dev-oven dev-shelf: sync-build-number
	$(MAKE) -j5 serve-geo serve-api serve-tile watch-wasm web SOURCE=$(@:dev-%=%)
dev-native:
	pnpm --dir ts/client start
preview: sync-build-number
	$(MAKE) web-preview
preview-oven preview-shelf: sync-build-number
	$(MAKE) -j4 serve-geo serve-api serve-tile web-preview SOURCE=$(@:preview-%=%)

serve-tile:
	+cd rust && $(CURDIR)/script/env.sh cargo serve-tile $(if $(filter shelf,$(SOURCE)),--shelf ./out) $(ARGS)
watch-wasm:
	./ts/client/script/wasm-watch.py
web:
	PXL_TILE_SOURCE=$(SOURCE) pnpm --dir ts/client web
web-preview:
	PXL_TILE_SOURCE=$(SOURCE) pnpm --dir ts/client preview

# The binaries that make the repository's data and the dev servers, each
# rust/'s `cargo <name>` (rust/.cargo/config.toml) under the local databases'
# addresses and the servers' numbers (script/env.sh). Each works in the
# checkout's root, so ./out and what ARGS names are the root's:
#
#   make bake-terrain ARGS="--shard 01033210 --step leaf"
#   make bake-geoid ARGS=egm2008-5.pgm
TOOLS := bake-terrain bake-place bake-division bake-earth bake-star bake-geoid ingest-overture \
	download-pack download-moon download-earth download-star ingest-asset render-brand \
	serve-geo serve-api
.PHONY: $(TOOLS)
$(TOOLS):
	+cd rust && $(CURDIR)/script/env.sh cargo $@ $(ARGS)

# ts/client/.build_number, the number the deployment serves, which a build CI
# did not number is labeled after.
sync-build-number:
	@./script/build-number.py

# The deployment: the client built and published with the Worker
# (script/deploy.py). Does not upload what the bucket holds.
deploy:
	./script/deploy.py
