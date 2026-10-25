# Makefile
#
# Routes only. Each tree is a root of its own: ts/ as Deno's (ts/deno.jsonc),
# its scripts among its members; rust/ as Cargo's, its commands as aliases in
# rust/.cargo/config.toml; and cpp/ as Make's (cpp/Makefile). Here is a target
# a tree for each verb, the aggregates over them, and one order: the traces
# run after the guests they drive. The documents and the spec are formatted
# by deno fmt under deno.jsonc. GNU Make 3.81 is enough.
#
#   make -j8 check | test | build | test-gpu | fmt | lint
#   make check-ts, make test-cpp, ...   one tree's
#
# With -j the trees run in parallel; cpp/'s make and cargo share its job
# slots ($(MAKE), and + before cargo).

.PHONY: check check-doc check-ts check-cpp check-rust \
	build build-cpp build-rust \
	test test-ts test-cpp test-rust test-conformance \
	test-gpu test-gpu-web test-gpu-native \
	fmt fmt-doc fmt-ts fmt-rust \
	lint lint-ts

check: check-doc check-ts check-cpp check-rust
check-doc:
	deno fmt --check
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
test-cpp:
	$(MAKE) -C cpp test
test-rust:
	+cd rust && cargo test --all
test-conformance: build
	deno task --cwd ts test:conformance

# What needs a GPU: the web host in Chrome (ts/script/web-gpu.ts), and the
# native one on wgpu.
test-gpu: test-gpu-web test-gpu-native
test-gpu-web:
	./ts/script/web-gpu.ts
test-gpu-native:
	+cd rust && cargo test-gpu

fmt: fmt-doc fmt-ts fmt-rust
fmt-doc:
	deno fmt
fmt-ts:
	deno task --cwd ts fmt
fmt-rust:
	cd rust && cargo fmt --all

lint: lint-ts
lint-ts:
	deno task --cwd ts lint
