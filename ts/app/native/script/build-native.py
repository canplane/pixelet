#!/usr/bin/env python3
# Builds the guest for a device (rust/client-bracket/native) as the app is
# built, and the app links what it leaves: so the kernel in an app is always
# this tree's, built for the configuration and the system the app is. A
# kernel kept between builds let an app link one left from another
# configuration — a debug bundle's dev settings over a kernel built without
# them — or from before the Rust last changed.
#
# Two builds run it. Xcode, before it compiles the platform module
# (modules/platform/ios/PxlPlatform.podspec's script phase), with every input
# in its environment: CONFIGURATION, PLATFORM_NAME and
# PODS_CONFIGURATION_BUILD_DIR, one directory for each configuration and SDK.
# Gradle, before it merges the module's native libraries
# (modules/platform/android/build.gradle), with them as arguments:
#
#   build-native.py --android --variant release --out <jniLibs dir> --ndk <dir> --api 24
#
# An iPhone links a static library; an Android app loads a shared one, asked
# of cargo by name so an iPhone's build makes none.
#
# A release is built as ts/app/web/script/build-wasm.py builds the page's
# deploy, with the kernel's `dev` feature off, so an app built for release
# carries the kernel a deployed page does; any other configuration keeps its
# dev settings.
#
# Xcode and Gradle name no Python of their own, so this runs under the
# system's python3, which on macOS is 3.9: it keeps to what 3.9 reads.

import os
import shutil
import subprocess
import sys
from pathlib import Path

APP = Path(__file__).resolve().parent.parent


def repo_of(start):
    """The repository over `start`: the nearest directory up holding the
    TypeScript workspace's manifest, ts/pnpm-workspace.yaml, as pxl-tool finds it by
    its own manifest. It works in a tree copied without .git."""
    for d in [start, *start.parents]:
        if (d / "ts" / "pnpm-workspace.yaml").is_file():
            return d
    sys.exit(f"no repository over {start}")


RUST = repo_of(APP) / "rust"


def fail(message):
    print(f"[build-native] {message}", file=sys.stderr)
    sys.exit(1)


# The Rust target each SDK an iPhone app is built for runs: the device, and
# the simulator on an Apple silicon Mac.
IOS_TARGETS = {"iphoneos": "aarch64-apple-ios", "iphonesimulator": "aarch64-apple-ios-sim"}

# The ABI an Android build packs the library for, and its Rust target.
ANDROID_ABI = "arm64-v8a"
ANDROID_TARGET = "aarch64-linux-android"


def xcode():
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
        "published": need("CONFIGURATION") == "Release",
        "built": "libpxl_native.a",
        "linked": f"{out}/PxlNative/libpxl_native.a",
        # Where the guest's C header and module map are put for Swift to
        # import (`import PxlNative`), beside the library.
        "include": f"{out}/PxlNative/include",
        "cargo": ["build"],
        "env": env,
    }


def gradle(args):
    def need(name):
        return args.get(name) or fail(f"--{name} is not given")

    # The NDK's compiler for the API the app runs down to, which links the
    # library and compiles what any crate builds in C.
    bin_dir = f"{need('ndk')}/toolchains/llvm/prebuilt/darwin-x86_64/bin"
    clang = f"{bin_dir}/{ANDROID_TARGET}{need('api')}-clang"
    return {
        "target": ANDROID_TARGET,
        "published": need("variant") == "release",
        "built": "libpxl_native.so",
        "linked": f"{need('out')}/{ANDROID_ABI}/libpxl_native.so",
        "include": None,
        "cargo": ["rustc", "--crate-type", "cdylib"],
        "env": {
            **os.environ,
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER": clang,
            # Loadable on a device whose pages are 16 KB, which the NDK's own
            # libraries already are and a Rust link is not unless told.
            "CARGO_TARGET_AARCH64_LINUX_ANDROID_RUSTFLAGS": "-C link-arg=-Wl,-z,max-page-size=16384",
            "CC_aarch64_linux_android": clang,
            "AR_aarch64_linux_android": f"{bin_dir}/llvm-ar",
        },
    }


def mtime(path):
    return os.stat(path).st_mtime_ns if os.path.exists(path) else 0


def main():
    argv = sys.argv[1:]
    # `--name value` pairs, as Gradle passes them.
    args, i = {}, 0
    while i < len(argv):
        if argv[i].startswith("--") and argv[i] != "--android" and i + 1 < len(argv):
            args[argv[i][2:]] = argv[i + 1]
            i += 2
        else:
            i += 1
    build = gradle(args) if "--android" in argv else xcode()
    # A release takes `native`, any other configuration `native-watch`, which a
    # change rebuilds in part (rust/Cargo.toml). Each in its own directory, so
    # building one never starts the other over.
    profile = "native" if build["published"] else "native-watch"

    code = subprocess.run(
        [*["cargo", *build["cargo"]], "--profile", profile, "--package", "pxl-native", "--target", build["target"], *(["--no-default-features"] if build["published"] else [])],
        cwd=RUST,
        env=build["env"],
    ).returncode
    if code != 0:
        return code

    # Copied only when cargo wrote a new one: the app relinks whenever the
    # library it takes is newer than the one it has.
    # Cargo's own rule: CARGO_TARGET_DIR, else the workspace's target/.
    target_dir = Path(os.environ["CARGO_TARGET_DIR"]) if os.environ.get("CARGO_TARGET_DIR") else RUST / "target"
    built = target_dir / build["target"] / profile / build["built"]
    linked = Path(build["linked"])
    if mtime(built) > mtime(linked):
        linked.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(built, linked)

    # The header beside it, so the pod names no path out of this app: written
    # only where it differs, as a newer one would rebuild every Swift module
    # that imports it.
    if build["include"]:
        include = Path(build["include"])
        include.mkdir(parents=True, exist_ok=True)
        for src in (RUST / "client-bracket/native/include").iterdir():
            dest = include / src.name
            text = src.read_bytes()
            if not dest.exists() or dest.read_bytes() != text:
                dest.write_bytes(text)

    print(f"[build-native] {'published' if build['published'] else 'dev'} {build['target']} -> {linked}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
