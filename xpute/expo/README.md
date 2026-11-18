# @xpute/expo

@xpute/expo is the native UI end: the Expo module through which a React Native
UI reaches xpute's native runtime (`runtime/native/README.md`). It starts the
runtime linked into the app on the guest's own thread, carries the UI's calls to
it and back, and tells the runtime the app's lifecycle itself, from Swift and
Kotlin, not through JavaScript. The web UI end is @xpute/runtime-web's page half
instead (`runtime/web/README.md`).

## The app's crate

An app installs the module, which Expo autolinks, and names its native crate
(`runtime/native/README.md`) in its `package.json`, under `xpute.guest`:

```json
"xpute": { "guest": {
  "workspace": "<the cargo workspace, from the app>",
  "runtime": "<the crate>",
  "profile": { "debug": "<cargo profile>", "release": "<cargo profile>" },
  "args": { "debug": ["<more of cargo's arguments>"], "release": [] }
} }
```

`profile` is cargo's `dev` and `release` where it is not given, and `args`,
features among them, nothing. The module builds the crate each time the app is
built, for the configuration and the system the app is built as, and links
what it built. On an iPhone its pod (`ios/Xpute.podspec`) asks cargo for a
static library and leaves it at
`$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib<crate>.a`, where the app links it.
On Android its Gradle module (`android/build.gradle`) asks for a shared one in
the task `buildGuestDebug` or `buildGuestRelease`, whose output is the library
the module packs among the app's `jniLibs` and the Kotlin class
`xpute.Runtime` loads by the crate's name. Cargo runs in the workspace, under
the toolchain it names, with the environment of the build that runs it, so
what cargo reads from its environment (`RUSTFLAGS`) an app sets there: a
build setting of the pod, or the task's `environment`. A step of the app's own
over the built library, a strip for one, is a script phase of a pod that
depends on `Xpute`, or a task between the build's and
`merge<Variant>JniLibFolders`.

Swift reaches the runtime through its C header (`ios/CXpute/xpute.h`). A
plugin's native UI end is a module of its own beside this one, which links
into the same crate.

The packages are their own source, and Metro reads only what lies under the
folders it watches: an app beside this tree names each package's directory in
its Metro config's `watchFolders`.

## The UI's end

The UI opens its end of the runtime (`open_ui`), mounts on it what it
implements, and then starts the runtime (`start`). The first start of a process
starts the guest with what is mounted on the UI's end by then, its named
handles and prefixes; each start attaches the UI under a new handle, one after
the first dropping the last UI's, as a reload's, and tells the guest a UI
attached. The UI hears the guest's stop and its trap (`stopped_on`, `trap_on`),
and each, as the UI begins to hear it, tells at once the last the runtime
keeps, so a UI that starts after the guest's end hears it too:

```ts
const ui = open_ui();
stopped_on((stop) => { /* show it */ });
trap_on((err) => { /* show it */ });
// Mount on `ui` what the UI implements, before the first `start`.
start();
```

What the runtime sends the UI comes in a batch on one notify, each message a
frame of the link's, and a buffer the guest lends is read and written through
a buffer the module makes over its span alone. Why the runtime outlives
its UI and keeps the guest's stop and trap is in
[ARCHITECTURE.md](../ARCHITECTURE.md), "The sides".
