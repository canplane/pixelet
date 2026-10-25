#!/usr/bin/env -S deno run -A
// Builds the guest for a device (rust/client-bracket/native) as the app is
// built, and the app links what it leaves: so the kernel in an app is always
// this tree's, built for the configuration and the system the app is. A
// kernel kept between builds let an app link one left from another
// configuration — a debug bundle's dev settings over a kernel built without
// them — or from before the Rust last changed.
//
// Two builds run it. Xcode, before it compiles the platform module
// (modules/platform/ios/PxlPlatform.podspec's script phase), with every input
// in its environment: CONFIGURATION, PLATFORM_NAME and
// PODS_CONFIGURATION_BUILD_DIR, one directory for each configuration and SDK.
// Gradle, before it merges the module's native libraries
// (modules/platform/android/build.gradle), with them as arguments:
//
//   build-native.ts --android --variant release --out <jniLibs dir> --ndk <dir> --api 24
//
// An iPhone links a static library; an Android app loads a shared one, asked
// of cargo by name so an iPhone's build makes none.
//
// A release is built as script/build-wasm.ts builds the page's deploy, with the
// kernel's `dev` feature off, so an app built for release carries the kernel a
// deployed page does; any other configuration keeps its dev settings.

import { REPO } from "@pxl/build/import-map.ts";

const RUST = new URL(`file://${REPO}/rust/`);

/** What one build asks for: the Rust target, whether it is a release, where
 * the library is put, and what cargo runs with. */
interface Build {
  target: string;
  published: boolean;
  /** The library under cargo's target directory, and where the app takes it from. */
  built: string;
  linked: string;
  /** Where the guest's C header and module map are put for Swift to import
   * (`import PxlNative`), beside the library; none where nothing imports it. */
  include?: string;
  /** cargo's subcommand and its own arguments before the shared ones. */
  cargo: string[];
  env: Record<string, string>;
}

function fail(message: string): never {
  console.error(`[build-native] ${message}`);
  Deno.exit(1);
}

/** The Rust target each SDK an iPhone app is built for runs: the device, and
 * the simulator on an Apple silicon Mac. */
const IOS_TARGETS: Record<string, string> = {
  iphoneos: "aarch64-apple-ios",
  iphonesimulator: "aarch64-apple-ios-sim",
};

function xcode(): Build {
  const need = (name: string) => Deno.env.get(name) ?? fail(`${name} is not set: this runs as the app's build phase, from Xcode or Gradle`);
  const sdk = need("PLATFORM_NAME");
  const target = IOS_TARGETS[sdk] ?? fail(`no Rust target for the SDK ${sdk}`);
  const env = { ...Deno.env.toObject() };
  // Xcode's SDKROOT names the iPhone's SDK, and a build script cargo compiles
  // for this Mac would link against it; rustc finds each target's SDK itself.
  delete env.SDKROOT;
  return {
    target,
    published: need("CONFIGURATION") === "Release",
    built: `${target}/native/libpxl_native.a`,
    linked: `${need("PODS_CONFIGURATION_BUILD_DIR")}/PxlNative/libpxl_native.a`,
    include: `${need("PODS_CONFIGURATION_BUILD_DIR")}/PxlNative/include`,
    cargo: ["build"],
    env,
  };
}

/** The ABI an Android build packs the library for, and its Rust target. */
const ANDROID_ABI = "arm64-v8a";
const ANDROID_TARGET = "aarch64-linux-android";

function gradle(args: Record<string, string | undefined>): Build {
  const need = (name: string) => args[name] ?? fail(`--${name} is not given`);
  // The NDK's compiler for the API the app runs down to, which links the
  // library and compiles what any crate builds in C.
  const bin = `${need("ndk")}/toolchains/llvm/prebuilt/darwin-x86_64/bin`;
  const clang = `${bin}/${ANDROID_TARGET}${need("api")}-clang`;
  return {
    target: ANDROID_TARGET,
    published: need("variant") === "release",
    built: `${ANDROID_TARGET}/native/libpxl_native.so`,
    linked: `${need("out")}/${ANDROID_ABI}/libpxl_native.so`,
    cargo: ["rustc", "--crate-type", "cdylib"],
    env: {
      ...Deno.env.toObject(),
      CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER: clang,
      // Loadable on a device whose pages are 16 KB, which the NDK's own
      // libraries already are and a Rust link is not unless told.
      CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS: "-C link-arg=-Wl,-z,max-page-size=16384",
      CC_aarch64_linux_android: clang,
      AR_aarch64_linux_android: `${bin}/llvm-ar`,
    },
  };
}

/** `--name value` pairs, as Gradle passes them. */
const args: Record<string, string | undefined> = {};
for (let i = 0; i < Deno.args.length; i++) {
  const name = Deno.args[i].match(/^--(.+)$/)?.[1];
  if (name && name !== "android") args[name] = Deno.args[++i];
}
const build = Deno.args.includes("--android") ? gradle(args) : xcode();

const { code } = await new Deno.Command("cargo", {
  args: [...build.cargo, "--profile", "native", "--package", "pxl-native", "--target", build.target, ...(build.published ? ["--no-default-features"] : [])],
  cwd: RUST,
  env: build.env,
  clearEnv: true,
  stdout: "inherit",
  stderr: "inherit",
}).output();
if (code !== 0) Deno.exit(code);

// Copied only when cargo wrote a new one: the app relinks whenever the
// library it takes is newer than the one it has.
// Cargo's own rule: CARGO_TARGET_DIR, else the workspace's target/.
const targetDir = Deno.env.get("CARGO_TARGET_DIR") ? new URL(`file://${Deno.env.get("CARGO_TARGET_DIR")}/`) : new URL("target/", RUST);
const built = new URL(build.built, targetDir).pathname;
const mtime = (path: string) => Deno.stat(path).then((s) => s.mtime?.getTime() ?? 0, () => 0);
if ((await mtime(built)) > (await mtime(build.linked))) {
  await Deno.mkdir(build.linked.slice(0, build.linked.lastIndexOf("/")), { recursive: true });
  await Deno.copyFile(built, build.linked);
}
// The header beside it, so the pod names no path out of this app: written
// only where it differs, as a newer one would rebuild every Swift module
// that imports it.
if (build.include) {
  const from = new URL("client-bracket/native/include/", RUST);
  await Deno.mkdir(build.include, { recursive: true });
  for await (const entry of Deno.readDir(from)) {
    const text = await Deno.readTextFile(new URL(entry.name, from));
    const to = `${build.include}/${entry.name}`;
    if ((await Deno.readTextFile(to).catch(() => null)) !== text) await Deno.writeTextFile(to, text);
  }
}
console.log(`[build-native] ${build.published ? "published" : "dev"} ${build.target} -> ${build.linked}`);
