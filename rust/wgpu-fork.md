# The native GPU host's wgpu

A native GPU host draws with wgpu, taken from the fork
[canplane/fork.wgpu](https://github.com/canplane/fork.wgpu), branch `xpute-30.0.1`,
which is wgpu 30.0.1 with three commits on it. The program's Cargo workspace names
its revision in `[patch.crates-io]`, and every crate of wgpu's comes from it so that they stay one set.

## One command buffer an encoder

Upstream's wgpu-core learns a pass's barriers only once it has validated the pass,
and the barriers must run before it. So it encodes each pass into a hal command
buffer of its own, and the barriers into another submitted ahead of it. A frame of
tens of passes then pays for two command buffers each, and the drivers charge for
that: Metal holds about 2.3 MiB for each `MTLCommandBuffer` until it completes, and
on Vulkan each pool, reset without `RELEASE_RESOURCES` and reused, keeps the memory
of its heaviest frame.

In the fork a pass is recorded on the CPU instead (`hal::PassRecorder`, in
`wgpu-hal/src/dynamic/record.rs`). Once it is validated, its barriers are encoded
into the encoder's open command buffer and the recording is replayed after them, so
an encoder is one hal command buffer on every backend. wgpu's transit, present and
deferred query resolves still take their own. It is the
[prototype](https://github.com/pshtif/wgpu/compare/v30.0.1...pass-encoding-prototype)
of a [comment](https://github.com/gfx-rs/wgpu/issues/5738#issuecomment-6232144528)
on [gfx-rs/wgpu#5738](https://github.com/gfx-rs/wgpu/issues/5738), taken as it is.

The recorder holds resources as `&'static` references made from shorter borrows by a
`transmute`. That is sound only because wgpu-core replays before the borrows end,
every resource of the pass being kept alive by its trackers and the snatch guard.
The cost is a walk over each pass's commands on the CPU.

## The iOS Simulator's Metal

`wgpu-hal/src/metal/adapter.rs` carries two facts Metal cannot be asked in the
Simulator, decided by `target_abi = "sim"`: that indirect calls run, and that
constant buffer offsets align to 256 bytes
([gfx-rs/wgpu#10189](https://github.com/gfx-rs/wgpu/issues/10189)).

## Metal's pipeline cache

Upstream's wgpu keeps a pipeline cache on Vulkan alone; on Metal its cache is
empty ([gfx-rs/wgpu#8526](https://github.com/gfx-rs/wgpu/issues/8526)). In the
fork a Metal cache is a binary archive (`MTLBinaryArchive`, in
`wgpu-hal/src/metal/device.rs`), and `Features::PIPELINE_CACHE` is reported where
the device takes one. A standard render pipeline and a compute pipeline look
their functions up in the archive first and add them to it once made. Metal
loads and serializes an archive only at a URL, so the data crosses through a
file of its own under the system's temporary directory. A cache's validation key
is the device's name and the system's version, and `wgpu-core` and
`pipeline_cache_key` tell a Metal GPU by its name, since Metal names no vendor
or device by number. It is the design proposed on that issue. xpute's native GPU
host keeps the cache in the app's caches (xpute-gpu-native-host `cache.rs`), on
Vulkan and Metal alike.

## When wgpu is upgraded

The fork's three commits are rebased onto the new release, on a branch of its own, and
a commit upstream has made unneeded is dropped. Where none is left the
`[patch.crates-io]` tables go. wgpu-core's pass encoding is what the recording
depends on, so the order of a pass's timestamp resets and writes and its deferred
resolves is read again, and `make -j8 test-gpu` is run. The recording's effect shows
on an Android phone as the `Graphics` line of `adb shell dumpsys meminfo <package>`.
