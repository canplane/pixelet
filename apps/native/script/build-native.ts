#!/usr/bin/env -S deno run -A
// Builds the Rust core for a device (crates/pxl/web/native) as the app is
// built. Xcode runs this before it compiles the platform module
// (modules/platform/ios/PxlPlatform.podspec's script phase), and the app links
// what it leaves: so the kernel in an app is always this tree's, built for the
// configuration and the SDK the app is. A kernel kept between builds let an
// app link one left from another configuration — a debug bundle's dev settings
// over a kernel built without them — or from before the Rust last changed.
//
// A release is built as script/build-wasm.ts builds the page's deploy, with the
// kernel's `dev` feature off, so an app built for release carries the kernel a
// deployed page does; any other configuration keeps its dev settings.
//
// Every input is Xcode's: CONFIGURATION, PLATFORM_NAME and
// PODS_CONFIGURATION_BUILD_DIR, which is one directory for each configuration
// and SDK, so each keeps its own library.

const CRATES = new URL("../../../crates/", import.meta.url);

/** The Rust target each SDK the app is built for runs: the device, and the
 * simulator on an Apple silicon Mac. */
const TARGETS: Record<string, string> = {
  iphoneos: "aarch64-apple-ios",
  iphonesimulator: "aarch64-apple-ios-sim",
};

function need(name: string): string {
  const value = Deno.env.get(name);
  if (!value) {
    console.error(`[build-native] ${name} is not set: this runs as the app's build phase, from Xcode`);
    Deno.exit(1);
  }
  return value;
}

const configuration = need("CONFIGURATION");
const sdk = need("PLATFORM_NAME");
const target = TARGETS[sdk];
if (!target) {
  console.error(`[build-native] no Rust target for the SDK ${sdk}`);
  Deno.exit(1);
}
const published = configuration === "Release";
const out = `${need("PODS_CONFIGURATION_BUILD_DIR")}/PxlNative`;

// Xcode's SDKROOT names the iPhone's SDK, and a build script cargo compiles
// for this Mac would link against it; rustc finds each target's SDK itself.
const env = { ...Deno.env.toObject() };
delete env.SDKROOT;
const { code } = await new Deno.Command("cargo", {
  args: ["build", "--profile", "native", "--package", "pxl-native", "--target", target, ...(published ? ["--no-default-features"] : [])],
  cwd: CRATES,
  env,
  clearEnv: true,
  stdout: "inherit",
  stderr: "inherit",
}).output();
if (code !== 0) Deno.exit(code);

// Copied only when cargo wrote a new one: the app relinks whenever the
// library it links is newer than it.
const built = new URL(`target/${target}/native/libpxl_native.a`, CRATES).pathname;
const linked = `${out}/libpxl_native.a`;
const mtime = (path: string) => Deno.stat(path).then((s) => s.mtime?.getTime() ?? 0, () => 0);
if ((await mtime(built)) > (await mtime(linked))) {
  await Deno.mkdir(out, { recursive: true });
  await Deno.copyFile(built, linked);
}
console.log(`[build-native] ${published ? "published" : "dev"} ${target} -> ${linked}`);
