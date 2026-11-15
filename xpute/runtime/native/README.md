# xpute-runtime-native

xpute-runtime-native is xpute's runtime on a device, as @xpute/runtime-web is
a page's. The guest is a library linked into the app, and the runtime runs it
on a thread of its own: that thread waits on one queue and enters the guest
when work arrives, and every other thread only waits long and pushes into the
queue. It gives its host what the host needs of it, the way back to the
guest's thread, the timers there, where a stop is told and its writer, which
alone writes to the platform's output.

A device links one Rust crate, whichever language its guest is in: the guest,
the runtime and the function that starts it. That function is given the
directory the platform keeps the app's caches in and answers a `Boot`: the stack
the guest's thread takes, the program's servers and the bindings' `IMPORTS` and
`CALLS`. `native_abi!` exports the runtime over it under xpute's C names
(`expo/ios/CXpute/xpute.h`), and on Android under JNI's for the Kotlin class
`xpute.Runtime`. Here a plugin of the program's serves beside xpute's host:

```rust
fn start(cache_dir: PathBuf) -> Boot {
    Boot {
        stack_sz: 8 << 20,
        servers: vec![
            Box::new(stop::serve),
            Box::new(clock::serve),
            Box::new(move |side| view::serve(side, Some(cache_dir))), // a plugin's
        ],
        imports: abi::IMPORTS,
        calls: abi::CALLS,
    }
}

xpute_runtime_native::native_abi!(start);
```

The runtime runs the steps a page's does, which
[ARCHITECTURE.md](../../ARCHITECTURE.md) gives in "The sides", with the
stand-ins of what the UI serves in `remote.rs`, and then the loop for the
guest's life. Its UI end on the JavaScript thread is `@xpute/expo`
(`expo/README.md`), which starts it; how the runtime carries that UI to the
guest, and why it outlives the UI, is in the same section.
