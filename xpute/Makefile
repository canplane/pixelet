# Makefile
#
# Routes only. Each tree is a root of its own: ts/ as Deno's, and script/,
# the repository's own scripts, as another; rust/ as Cargo's, its commands as
# aliases in rust/.cargo/config.toml; and cpp/ as Make's (cpp/Makefile). Here
# is a target a tree for each verb, the aggregates over them, and two
# orders: the traces run after the guests they drive, and cpp/'s tests,
# which build those guests too, after build has, so two makes never write
# one object at once. The documents and the spec are formatted by deno fmt
# under deno.jsonc. GNU Make 3.81 is enough.
#
#   make -j8 check | test | build | test-gpu | fmt | lint
#   make check-ts, make test-cpp, ...   one tree's
#
# With -j the trees run in parallel; cpp/'s make and cargo share its job
# slots ($(MAKE), and + before cargo).

.PHONY: check check-doc check-script check-ts check-cpp check-rust \
	build build-cpp build-rust \
	test test-ts test-cpp test-rust test-conformance \
	test-gpu test-gpu-web test-gpu-native \
	fmt fmt-doc fmt-script fmt-ts fmt-rust \
	lint lint-script lint-ts

check: check-doc check-script check-ts check-cpp check-rust
check-doc:
	deno fmt --check
check-script:
	deno task --cwd script check
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

# What needs a GPU: the web host in Chrome (ts/gpu/script/web-gpu.ts), and the
# native one on wgpu.
test-gpu: test-gpu-web test-gpu-native
test-gpu-web:
	deno task --cwd ts test:gpu:web
test-gpu-native:
	+cd rust && cargo test-gpu

fmt: fmt-doc fmt-script fmt-ts fmt-rust
fmt-doc:
	deno fmt
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
