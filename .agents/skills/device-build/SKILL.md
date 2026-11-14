---
name: device-build
description: Build the device app (ts/client) and put it on an iPhone, the iOS simulator, an Android emulator or an Android phone — the one-time setup of each machine, the Debug and Release builds, install and launch, and the failures each has had. Use when a device build is asked for, when one fails, or when a machine is set up for one. Running the page is CONTRIBUTING.md's.
---

# Building for a device

Prebuild first: `ts/client/ios/` and `ts/client/android/` come from
`expo prebuild` and are not committed. Each platform's build builds the core
with its runtime, `pxl-runtime` (`ts/client/package.json` `xpute.guest.runtime`),
through the app's `ts/client/script/core-build.py`, which `PxlPlatform.podspec`
and `build.gradle` (`ts/client/modules/platform/`) run.

After the app moves, pnpm's links are remade or a config plugin is dropped from `app.config.ts`, prebuild again with `--clean` and run `pod install` again.

A **Release** build embeds the JavaScript bundle. A **Debug** build loads it
from Metro (`make dev-native`) on port 8081. The running Metro serves your tree
and started after `ts/client/babel.config.ts` last changed, or is restarted with
`--clear`.

Before a Release, bring the build number up to date:

```bash
make sync-build-number
```

## iOS

### Once per machine

- Xcode with a signing team for a phone, CocoaPods, and the Rust targets, which
  `rust/rust-toolchain.toml` installs.
- Node, as `ts/client/package.json`'s `engines` asks, on `PATH` for
  `pod install`, and named for Xcode's script phases in
  `ts/client/ios/.xcode.env.local` (`export NODE_BINARY=…`), which prebuild
  does not write.

### Generate

```bash
make install && (cd ts/client && pnpm exec expo prebuild --platform ios --no-install)
(cd ts/client/ios && pod install)
```

Run both again after a file is added under `modules/platform/` or
`xpute/native/runtime/expo/ios/` or `xpute/gpu/native/runtime/expo/ios/` (only `pod install` adds a new Swift file to the pod),
after `app.config.ts` or a plugin changes, or after a dependency is added.

### Build, install, launch

Ask the user before taking a phone or the simulator (AGENTS.md).

From `ts/client/ios`. A phone has different ids for xcodebuild and
devicectl: `xcrun xctrace list devices` gives the first, `xcrun devicectl list
devices` the second.

```bash
xcodebuild -workspace pixelet.xcworkspace -scheme pixelet -configuration Release \
  -destination 'id=<xcodebuild id>' -derivedDataPath build/derived \
  DEVELOPMENT_TEAM=<team> CODE_SIGN_STYLE=Automatic -allowProvisioningUpdates -quiet
../../../script/perf/ios.py xcrun devicectl device install app --device <devicectl id> build/derived/Build/Products/Release-iphoneos/pixelet.app
../../../script/perf/ios.py xcrun devicectl device process launch --terminate-existing --device <devicectl id> me.plei.pixelet
```

Add `--console` to the launch to stream the app's stdout, where the core's
last messages go (`[pxl] …`).

### The simulator

Xcode's simulator window is Device Hub, `Contents/Applications/DeviceHub.app`
in Xcode (`com.apple.dt.Devices`). From `ts/client/ios`, with an id from
`xcrun simctl list devices available`:

```bash
xcrun simctl boot <id> && open -g -b com.apple.dt.Devices
xcodebuild -workspace pixelet.xcworkspace -scheme pixelet -configuration Debug \
  -destination 'id=<id>' -derivedDataPath build/derived -quiet
../../../script/perf/mac.py xcrun simctl install booted build/derived/Build/Products/Debug-iphonesimulator/pixelet.app
xcrun simctl privacy booted grant location me.plei.pixelet
xcrun simctl location booted set 37.2741,127.0437
../../../script/perf/mac.py xcrun simctl launch booted me.plei.pixelet
```

`xcrun simctl io booted screenshot <file>.png` for a look.

## Android

### Once per machine

- Android Studio. In its SDK Manager: **SDK Tools** — NDK (Side by side), CMake,
  Android SDK Command-line Tools; **SDK Platforms** — the platform and an **ARM
  64 v8a** system image (the 16 KB page size one) for an emulator.
- An AVD from that image (Device Manager), or a phone with USB debugging on.
- **JDK 17.** A newer one (Android Studio ships 25) fails the native build of
  expo-modules-core with "A restricted method in java.lang.System has been
  called". `ts/client/.java-version` pins 17 for jenv: run
  `jenv add <JDK 17 home>` once, then run Gradle as `jenv exec ./gradlew …`.
  Opening the project in Android Studio rewrites
  `android/gradle/gradle-daemon-jvm.properties` for its Java 25, which overrides
  `JAVA_HOME`; prebuild again to undo it.
- The Rust target `aarch64-linux-android`, which `rust/rust-toolchain.toml`
  installs.

### Generate

```bash
make install && (cd ts/client && pnpm exec expo prebuild --platform android --no-install)
```

### Build, install, launch

Ask the user before taking a phone or an emulator (AGENTS.md).

From `ts/client/android`, with `ANDROID_HOME=~/Library/Android/sdk` and the
platform tools' `adb`:

```bash
jenv exec ./gradlew :app:assembleRelease
../../../script/perf/android.py adb install -r app/build/outputs/apk/release/app-release.apk
../../../script/perf/android.py adb shell am start -S -n me.plei.pixelet/.MainActivity
```

On an emulator, which draws on the Mac's GPU, the two run under
script/perf/mac.py in place of android.py.
`jenv exec ./gradlew :app:installRelease` builds and installs in one step, under the same
module. The core's last
messages and panics go to logcat under the tag `pxl`:
`adb logcat -d | grep -E "FATAL| pxl |ReactNativeJS"`.

On an emulator, grant location and move it to where there is data:

```bash
adb shell pm grant me.plei.pixelet android.permission.ACCESS_FINE_LOCATION
adb emu geo fix 126.9780 37.5665
```
`adb exec-out screencap -p > shot.png` for a look.

A headset's build is not this one: `ts/xr/README.md`.
