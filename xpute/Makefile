# Makefile
#
# Routes only. Each tree is a root of its own: ts/ as pnpm's; rust/ as
# Cargo's, its commands as aliases in rust/.cargo/config.toml; and cpp/ as
# Make's (cpp/Makefile). script/ is the repository's own, run by path. Here
# is a target a tree for each verb, the aggregates over them, and two
# orders: the traces run after the guests they drive, and cpp/'s tests,
# which build those guests too, after build has, so two makes never write
# one object at once. Only code is formatted, each tree's by its own
# formatter; the documents and the spec are not. GNU Make 3.81 is enough.
#
#   make install | -j8 check | test | build | test-gpu | fmt | lint
#   make check-ts, make test-cpp, ...   one tree's
#
# With -j the trees run in parallel; cpp/'s make and cargo share its job
# slots ($(MAKE), and + before cargo).

.PHONY: install install-ts \
	check check-ts check-cpp check-rust \
	build build-cpp build-rust \
	test test-ts test-cpp test-rust test-conformance \
	test-gpu test-gpu-web test-gpu-native \
	fmt fmt-ts fmt-rust \
	lint lint-ts

# ts/'s packages, pnpm's; rust/'s and cpp/'s are fetched by their builds.
install: install-ts
install-ts:
	pnpm --dir ts install

check: check-ts check-cpp check-rust
check-ts:
	pnpm --dir ts check
check-cpp:
	$(MAKE) -C cpp check
check-rust:
	+cd rust && cargo fmt --all --check && cargo lint

# The one thing xpute builds: the guest the contract's traces drive, in Rust
# and in C++, each a native library and a WebAssembly module.
build: build-cpp build-rust
build-cpp:
	$(MAKE) -C cpp guest wasm
build-rust:
	+cd rust && cargo guest && cargo guest-wasm

test: test-ts test-cpp test-rust test-conformance
test-ts:
	pnpm --dir ts test
test-cpp: build-cpp
	$(MAKE) -C cpp test
test-rust:
	+cd rust && cargo test --all
test-conformance: build
	pnpm --dir ts test:conformance

# What needs a GPU: the web host in Chrome (ts/gpu/script/browser.ts), and the
# native one on wgpu.
test-gpu: test-gpu-web test-gpu-native
test-gpu-web:
	pnpm --dir ts test:gpu:web
test-gpu-native:
	+cd rust && cargo test-gpu

fmt: fmt-ts fmt-rust
fmt-ts:
	pnpm --dir ts fmt
fmt-rust:
	cd rust && cargo fmt --all

lint: lint-ts
lint-ts:
	pnpm --dir ts lint
