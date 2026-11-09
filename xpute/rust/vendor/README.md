# The vendored wgpu

A native GPU runtime draws with wgpu, and two of wgpu's crates are kept here, at
30.0.1, because each needs a change upstream does not have. Both Cargo workspaces
take them through `[patch.crates-io]`, in `xpute/rust/Cargo.toml` and in the
program's own workspace. Everything not described below is upstream's, which a
`diff -r` of `src` against the crate in Cargo's registry shows.

## One command buffer an encoder

Upstream's wgpu-core learns a pass's barriers only once it has validated the
pass, and the barriers must run before it. So it encodes each pass into a hal
command buffer of its own, and the barriers into another submitted ahead of it. A
frame of tens of passes then pays for two command buffers each, and the drivers
charge for that: Metal holds about 2.3 MiB for each `MTLCommandBuffer` until it
completes, and on Vulkan each pool, reset without `RELEASE_RESOURCES` and reused,
keeps the memory of its heaviest frame.

Here a pass is recorded on the CPU instead (`hal::PassRecorder`, in
`wgpu-hal/src/dynamic/record.rs`). Once it is validated, its barriers are encoded
into the encoder's open command buffer and the recording is replayed after them,
so an encoder is one hal command buffer on every backend. wgpu's transit, present
and deferred query resolves still take their own. The change is in
`wgpu-core/src/command/{compute,render,mod}.rs` and `wgpu-hal/src/dynamic/`, with
the export in `wgpu-hal/src/lib.rs`. It is the
[prototype](https://github.com/pshtif/wgpu/compare/v30.0.1...pass-encoding-prototype)
of a [comment](https://github.com/gfx-rs/wgpu/issues/5738#issuecomment-6232144528)
on [gfx-rs/wgpu#5738](https://github.com/gfx-rs/wgpu/issues/5738), taken as it is.

The recorder holds resources as `&'static` references made from shorter borrows by
a `transmute`. That is sound only because wgpu-core replays before the borrows
end, every resource of the pass being kept alive by its trackers and the snatch
guard. The cost is a walk over each pass's commands on the CPU. The form upstream
would take is two walks over the pass in wgpu-core, the first to validate and the
second to encode, with no recorder.

## The iOS Simulator's Metal

`wgpu-hal/src/metal/adapter.rs` carries two facts Metal cannot be asked in the
Simulator, decided by `target_abi = "sim"`: that indirect calls run, and that
constant buffer offsets align to 256 bytes
([gfx-rs/wgpu#10189](https://github.com/gfx-rs/wgpu/issues/10189)).

## When wgpu is upgraded

Where upstream has fixed either, its part goes from here, and a crate with nothing
left goes from both `[patch.crates-io]` tables. What stays is applied again to the
new version. wgpu-core's pass encoding is what the recording depends on, so the
order of a pass's timestamp resets and writes and its deferred resolves is read
again, and `make -j8 test-gpu` is run. The recording's effect shows on an Android
phone as the `Graphics` line of `adb shell dumpsys meminfo <package>`.
