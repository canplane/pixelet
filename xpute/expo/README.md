# @xpute/expo

@xpute/expo is the native UI end: the Expo module through which a React Native
UI reaches xpute's native runtime (`runtime/native/README.md`). It starts the
runtime linked into the app on the guest's own thread, carries the UI's calls to
it and back, and tells the runtime the app's lifecycle itself, from Swift and
Kotlin, not through JavaScript. The web UI end is @xpute/runtime-web's page half
instead (`runtime/web/README.md`).

## The app's crate

An app installs the module, which Expo autolinks, and names in its
`package.json` under `xpute.guest.runtime` the Rust crate a native build links:
its guest, and the function that starts it, which the crate exports the runtime
over (`native_abi!`). The app builds that crate for the device as it builds, and
the module links it: a static library at
`$PODS_CONFIGURATION_BUILD_DIR/Xpute/lib<crate>.a` on an iPhone
(`ios/Xpute.podspec`), and a shared one among the app's `jniLibs` on Android,
which the Kotlin class `xpute.Runtime` loads by the crate's name
(`android/build.gradle`). Swift reaches the runtime through its C header
(`ios/CXpute/xpute.h`). A plugin's native UI end is a module of its own beside
this one, which links into the same crate.

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
