#!/usr/bin/env -S deno run -A
// Builds the Rust core for a device (crates/pxl/web/native) and puts it where
// the app's platform module links it from: one xcframework holding the iPhone's
// library and the simulator's, each with the header Swift imports it by
// (`import PxlNative`). Run before the app is built; the framework is not
// committed.
//
//   ./script/build-native.ts               # a kernel with its dev settings
//   ./script/build-native.ts --published   # without, as the page's deploy is
//
// The published kernel is built as script/build-wasm.ts builds the page's,
// with the kernel's `dev` feature off, so an app built for release carries
// the same kernel a deployed page does.

const CRATES = new URL("../../../crates/", import.meta.url);
const INCLUDE = new URL("pxl/web/native/include/", CRATES);
const OUT = new URL("../modules/platform/ios/PxlNative.xcframework", import.meta.url);

/** The targets an iPhone app is built for: the device, and the simulator on
 * an Apple silicon Mac. */
const TARGETS = ["aarch64-apple-ios", "aarch64-apple-ios-sim"];

async function run(cmd: string, args: string[], cwd?: URL): Promise<void> {
  const { code } = await new Deno.Command(cmd, { args, cwd, stdout: "inherit", stderr: "inherit" }).output();
  if (code !== 0) Deno.exit(code);
}

const published = Deno.args.includes("--published");

for (const target of TARGETS) {
  await run("cargo", ["build", "--profile", "native", "--package", "pxl-native", "--target", target, ...(published ? ["--no-default-features"] : [])], CRATES);
}

// xcodebuild refuses to write over a framework that is there.
await Deno.remove(OUT, { recursive: true }).catch(() => {});
await run("xcodebuild", [
  "-create-xcframework",
  ...TARGETS.flatMap((t) => ["-library", new URL(`target/${t}/native/libpxl_native.a`, CRATES).pathname, "-headers", INCLUDE.pathname]),
  "-output",
  OUT.pathname,
]);
console.log(`[build-native] ${published ? "published " : ""}${TARGETS.join(", ")} -> ${OUT.pathname}`);
