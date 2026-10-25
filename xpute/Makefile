# Makefile
#
# Routes only. Each tree is a root of its own: ts/ as Deno's; rust/ as
# Cargo's, its commands as aliases in rust/.cargo/config.toml; and cpp/ as
# Make's (cpp/Makefile). script/ is the repository's own, run by path. Here
# is a target a tree for each verb, the aggregates over them, and two
# orders: the traces run after the guests they drive, and cpp/'s tests,
# which build those guests too, after build has, so two makes never write
# one object at once. Only code is formatted, each tree's by its own
# formatter; the documents and the spec are not. GNU Make 3.81 is enough.
#
#   make -j8 check | test | build | test-gpu | fmt | lint
#   make check-ts, make test-cpp, ...   one tree's
#
# With -j the trees run in parallel; cpp/'s make and cargo share its job
# slots ($(MAKE), and + before cargo).

.PHONY: check check-ts check-cpp check-rust \
	build build-cpp build-rust \
	test test-ts test-cpp test-rust test-conformance \
	test-gpu test-gpu-web test-gpu-native \
	fmt fmt-ts fmt-rust \
	lint lint-ts

check: check-ts check-cpp check-rust
check-ts:
	deno task --cwd ts check
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
	deno task --cwd ts test
test-cpp: build-cpp
	$(MAKE) -C cpp test
test-rust:
	+cd rust && cargo test --all
test-conformance: build
	deno task --cwd ts test:conformance

# What needs a GPU: the web host in Chrome (ts/gpu/script/browser.ts), and the
# native one on wgpu.
test-gpu: test-gpu-web test-gpu-native
test-gpu-web:
	deno task --cwd ts test:gpu:web
test-gpu-native:
	+cd rust && cargo test-gpu

fmt: fmt-ts fmt-rust
fmt-ts:
	deno task --cwd ts fmt
fmt-rust:
	cd rust && cargo fmt --all

lint: lint-ts
lint-ts:
	deno task --cwd ts lint
