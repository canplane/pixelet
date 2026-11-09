#!/usr/bin/env python3
# Builds an app's core crate for a device as the app is built, and the app
# links what it leaves: so the core in an app is always the tree's, built for
# the configuration and the system the app is.
#
# The app names its core in its package.json:
#
#   "xpute": { "core": {
#     "workspace": "<the cargo workspace, from the app>",
#     "package": "<the crate>",
#     "profile": { "debug": "<cargo profile>", "release": "<cargo profile>" },
#     "args": { "debug": [...], "release": [...] }
#   } }
#
# `profile` defaults to cargo's dev and release, `args` (more of cargo's
# arguments, features among them) to none.
#
# Two builds run it. Xcode, before it compiles xpute's module
# (ios/Xpute.podspec's script phase), with every input in its environment:
# CONFIGURATION, PLATFORM_NAME and PODS_CONFIGURATION_BUILD_DIR, one directory
# for each configuration and SDK. Gradle, before it merges the module's native
# libraries (android/build.gradle), with them as arguments:
#
#   core-build.py --app <dir> --android --variant release --out <jniLibs dir> --ndk <dir> --api 24
#
# An iPhone links a static library; an Android app loads a shared one. Each
# is asked of cargo by name, so a build makes only the library its device
# takes, whatever crate types the core declares.
#
# Xcode and Gradle name no Python of their own, so this runs under the
# system's python3, which on macOS is 3.9: it keeps to what 3.9 reads.

import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

# These are xpute's C ABI for the core and its GPU runtime, which Swift imports
# as one module (`import CXpute`, ios/CXpute/module.modulemap).
XPUTE = Path(__file__).resolve().parents[3]
HEADERS = [
    XPUTE / "rust/runtime/include/xpute.h",
    XPUTE / "rust/gpu/include/xpute_gpu.h",
    XPUTE / "ts/native/ios/CXpute/module.modulemap",
]


def fail(message):
    print(f"[core-build] {message}", file=sys.stderr)
    sys.exit(1)


# Each SDK an iPhone app is built for runs this Rust target: the device, and the
# simulator on an Apple silicon Mac.
IOS_TARGETS = {"iphoneos": "aarch64-apple-ios", "iphonesimulator": "aarch64-apple-ios-sim"}

# An Android build packs the library for this ABI, and this is its Rust target.
NUMBER_PATHS = Path(__file__).with_name("number-paths.py")
ANDROID_ABI = "arm64-v8a"
ANDROID_TARGET = "aarch64-linux-android"


def xcode(lib):
    def need(name):
        return os.environ.get(name) or fail(f"{name} is not set: this runs as the app's build phase, from Xcode or Gradle")

    sdk = need("PLATFORM_NAME")
    target = IOS_TARGETS.get(sdk) or fail(f"no Rust target for the SDK {sdk}")
    env = dict(os.environ)
    # Xcode's SDKROOT names the iPhone's SDK, and a build script cargo compiles
    # for this Mac would link against it; rustc finds each target's SDK itself.
    env.pop("SDKROOT", None)
    out = need("PODS_CONFIGURATION_BUILD_DIR")
    return {
        "target": target,
        "release": need("CONFIGURATION") == "Release",
        "built": f"lib{lib}.a",
        "linked": f"{out}/Xpute/lib{lib}.a",
        # The headers are put here for Swift to import, beside the library.
        "include": f"{out}/Xpute/include",
        "cargo": ["rustc", "--crate-type", "staticlib"],
        "env": env,
    }


def gradle(args, lib):
    def need(name):
        return args.get(name) or fail(f"--{name} is not given")

    # This is the NDK's compiler for the API the app runs down to, which links
    # the library and compiles what any crate builds in C.
    bin_dir = f"{need('ndk')}/toolchains/llvm/prebuilt/darwin-x86_64/bin"
    clang = f"{bin_dir}/{ANDROID_TARGET}{need('api')}-clang"
    return {
        "target": ANDROID_TARGET,
        "release": need("variant") == "release",
        "built": f"lib{lib}.so",
        "linked": f"{need('out')}/{ANDROID_ABI}/lib{lib}.so",
        "include": None,
        "cargo": ["rustc", "--crate-type", "cdylib"],
        "env": {
            **os.environ,
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER": clang,
            # This makes the library loadable on a device whose pages are 16 KB,
            # which the NDK's own libraries already are and a Rust link is not
            # unless told.
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS": "-C link-arg=-Wl,-z,max-page-size=16384",
            "CC_aarch64_linux_android": clang,
            "AR_aarch64_linux_android": f"{bin_dir}/llvm-ar",
        },
    }


def mtime(path):
    return os.stat(path).st_mtime_ns if os.path.exists(path) else 0


def write_changed(src, dest):
    """This writes only where it differs, as a newer header rebuilds every
    Swift module that imports it."""
    text = src.read_bytes()
    if not dest.exists() or dest.read_bytes() != text:
        dest.write_bytes(text)


def main():
    argv = sys.argv[1:]
    # The arguments are `--name value` pairs, as Gradle passes them.
    args, i = {}, 0
    while i < len(argv):
        if argv[i].startswith("--") and argv[i] != "--android" and i + 1 < len(argv):
            args[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            i += 1
    app = Path(args.get("app") or fail("--app is not given"))
    core = json.loads((app / "package.json").read_text()).get("xpute", {}).get("core") or fail(f"{app}/package.json names no xpute.core")
    workspace = (app / core["workspace"]).resolve()
    package = core["package"]
    lib = package.replace("-", "_")
    build = gradle(args, lib) if "--android" in argv else xcode(lib)
    which = "release" if build["release"] else "debug"
    if build["release"]:
        # A panic's location carries the directories it was built in. The
        # target's own variable, as RUSTFLAGS would replace what Gradle sets.
        env = build["env"]
        key = "CARGO_TARGET_" + build["target"].upper().replace("-", "_") + "_RUSTFLAGS"
        remaps = [f"--remap-path-prefix={p}=" for p in (workspace.parent, os.environ["HOME"])]
        env[key] = " ".join(f for f in [env.pop("RUSTFLAGS", None), env.get(key), *remaps] if f)
    profile = core.get("profile", {}).get(which, "release" if build["release"] else "dev")
    extra = core.get("args", {}).get(which, [])

    code = subprocess.run(
        ["cargo", *build["cargo"], "--profile", profile, "--package", package, "--target", build["target"], *extra],
        cwd=workspace,
        env=build["env"],
    ).returncode
    if code != 0:
        return code

    # The library is copied only when cargo wrote a new one: the app relinks
    # whenever the library it takes is newer than the one it has. Cargo's own
    # rule says where it lies: CARGO_TARGET_DIR, else the workspace's target/,
    # under the profile's directory, which cargo names `debug` for dev.
    target_dir = Path(os.environ["CARGO_TARGET_DIR"]) if os.environ.get("CARGO_TARGET_DIR") else workspace / "target"
    built = target_dir / build["target"] / ("debug" if profile == "dev" else profile) / build["built"]
    linked = Path(build["linked"])
    if mtime(built) > mtime(linked):
        linked.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(built, linked)
        if build["release"]:
            table = app / f"core.paths.{build['target']}.json"
            code = subprocess.run([sys.executable, str(NUMBER_PATHS), str(linked), str(table), str(workspace)]).returncode
            if code != 0:
                linked.unlink()
                return code

    # The headers lie beside it, so no pod names a path out of the app: xpute's
    # as their own module, and the crate's include/, where it has one.
    if build["include"]:
        include = Path(build["include"])
        (include / "CXpute").mkdir(parents=True, exist_ok=True)
        for src in HEADERS:
            write_changed(src, include / "CXpute" / src.name)
        metadata = json.loads(subprocess.run(["cargo", "metadata", "--no-deps", "--format-version", "1"], cwd=workspace, capture_output=True, check=True).stdout)
        manifest = next(p["manifest_path"] for p in metadata["packages"] if p["name"] == package)
        own = Path(manifest).parent / "include"
        if own.is_dir():
            for src in own.iterdir():
                write_changed(src, include / src.name)

    print(f"[core-build] {which} {package} {build['target']} -> {linked}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
