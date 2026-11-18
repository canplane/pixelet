# xr

The way in for the headsets, a Meta Quest and an Apple Vision Pro. Nothing
builds it yet, and it may not build at all. It is kept apart so that the
phone app in `ts/client` never bends to a headset: the phone takes the
libraries and the build a phone wants, and what a headset needs instead is
written here.

## What a headset needs that a phone does not

A Quest runs Horizon OS, an Android without Google Play Services. The phone
app reads its location through `expo-location`, which asks Play Services for
it; a Quest needs `expo-horizon-location`, a fork that reads the location
another way and that cannot raise the dialog that turns a phone's location
on. Its build also comes in two flavors, `mobile` and `quest`, which
`expo-horizon-core` adds, and only the `quest` one is this app's.

A Vision Pro runs the iPhone app unchanged, in a window, as the App Store
offers every iPhone app there. Nothing in the code is the Vision Pro's.

The core already has the way in for a headset's own view: the input
interface's `xr-pose` hands it a head pose, and the camera follows that pose
until `xr-pose-end` (`spec/client/core/wit/input.wit`,
`rust/client/core/src/domain/control/camera`). Nothing sends it yet.

## How it is meant to grow

This is to be an Expo app of its own beside `ts/client`, with its own native
libraries, so that a headset's library never links into the phone's build.
It takes the client's code whole and differs only where a headset does:

- `app.config.ts` is the phone app's table with Horizon OS's plugins in
  place of `expo-location`'s.
- `package.json` holds the libraries only a headset takes.
- A file of the client that a headset must replace is replaced here, at the
  client's path under `src/`, as `<x>.quest.ts` (or `<x>.vision.ts`, once a
  Vision Pro has an app of its own), which this app's bundler takes before
  the client's own. A library a headset swaps whole, as the location is, is
  swapped where this app's bundler resolves it, so the client names
  `expo-location` and nothing else.
- `plugin/quest-build.ts` tells React Native that the flavors' debug builds
  are debug builds.

It is already a package of `ts/pnpm-workspace.yaml`. To build it, give it a
Metro and a Babel config that extend the client's with the variants above, and
run `expo prebuild` here.

## On a headset

### A Meta Quest

Once this app builds, build the `quest` flavor from its generated `android/`
(`assembleQuestRelease`, or `assembleQuestDebug` to load from Metro) and use
`adb` as for a phone. A headset accepts `adb` once
developer mode is on (in the Meta Horizon phone app, with a Meta developer
account) and it is plugged in and trusted.

Without a headset, use Meta's Spatial Simulator. `npx metavr@latest init` signs
in to a Meta account; run `metavr` through npx:

```bash
npx metavr tools install spatialsim
npx metavr ssim start    # an emulator adb sees, emulator-5554 if it is the first
adb install -r app/build/outputs/apk/quest/release/app-quest-release.apk
adb shell cmd location set-location-enabled true
adb emu geo fix 126.9780 37.5665
```

### An Apple Vision Pro

The iPhone app runs unchanged on visionOS, in a window. In the simulator, from
`ts/client/ios`, once Xcode has the runtime
(`xcodebuild -downloadPlatform visionOS`):

```bash
xcodebuild -workspace pixelet.xcworkspace -scheme pixelet -configuration Release \
  -destination 'id=<Vision Pro simulator id>' -derivedDataPath build/derived
xcrun simctl install <id> build/derived/Build/Products/Release-iphonesimulator/pixelet.app
xcrun simctl location <id> set 37.5665,126.9780
xcrun simctl launch <id> me.plei.pixelet
```
