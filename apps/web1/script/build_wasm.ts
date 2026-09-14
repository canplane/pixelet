// @pxl/web/../script/build_wasm.ts
//
// Builds the Rust core's module (crates/pxl/wasm) and puts it where
// src/wasm/pxl.ts loads it from. Run by every task that loads the module —
// dev, build, test — so a checkout never runs against a stale or missing
// one; cargo's own cache makes a run with nothing changed cost a second.
//
// Usage (from apps/web):
//   deno run -A script/build_wasm.ts

const CRATES = new URL("../../../crates/", import.meta.url);
const TARGET = "wasm32-unknown-unknown";
const BUILT = new URL(`target/${TARGET}/release/pxl_wasm.wasm`, CRATES);
const OUT = new URL("../src/wasm/pxl.wasm", import.meta.url);

const cargo = new Deno.Command("cargo", {
  args: ["build", "--release", "--package", "pxl-wasm", "--target", TARGET],
  cwd: CRATES,
  stdout: "inherit",
  stderr: "inherit",
});

let status: Deno.CommandStatus;
try {
  status = await cargo.output();
} catch (err) {
  console.error(`[build_wasm] cargo did not run (${err}). The core needs a Rust toolchain with the ${TARGET} target: rustup target add ${TARGET}`);
  Deno.exit(1);
}
if (!status.success) Deno.exit(status.code);

await Deno.copyFile(BUILT, OUT);
console.log(`[build_wasm] ${(await Deno.stat(OUT)).size} bytes -> src/wasm/pxl.wasm`);
