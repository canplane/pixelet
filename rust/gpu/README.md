# The GPU

The GPU is pixelet's plugin of xpute: the one device whose work holds the
guest's memory past an answer. A frame is complete in itself, and once its
answer is in, the caller may write over everything it lent. A buffer the GPU
uploads from, or a word it writes when a request finishes, is still read or
written long after it was asked for, so the GPU needs a rule of its own for
who owns which bytes and when they come back. Its stream covers what pixelet
draws with, not all of WebGPU.

## Its interfaces

The GPU is declared in WIT as xpute's packages are, as `pxl:gpu`
(`spec/gpu/wit/`), and its bindings are written by pixelet's
`cargo spec` over xpute's own packages, before the core's, which take them.
The guest imports `gpu`, which runs the records it lends and gives it a
device, and `vsync`, which answers at the display's next frame, and serves
`surface`, which the UI opens to draw on.

## The stream

A renderer makes thousands of calls a frame, and none needs a reply. For
those the guest writes a **stream** of records (`spec/gpu/api.json5`,
its enumerations in `enum.json5`) in WebGPU's own vocabulary and lends it to
the host's `gpu.run`; until the answer, the records and the memory they name
are the host's. An address in a record is a u64, as the link's `lent` is,
written as two words. The device is answered by `get-device`, and a
completion the stream asked for lands as a **fence**, a word only the host
writes, set to the guest's **ticket** with release, so a late answer to a
dropped request never matches.

Both hosts admit a frame by one rule: the guest draws only while fewer of its
frames are unfinished than its frame latency (`pxl_gpu_guest::admit::frame`),
which the program sets in its host's `GpuConfig` and the host writes beside
its count in the device's block, so taking a surface's texture never blocks.
What a surface shows is lent by the UI beside the link, under the handle of a
surface the guest serves.

## Its parts

The Rust is three crates here, and the TypeScript one package, `@pxl/gpu`
(`ts/gpu/`), whose `host` and `expo` folders stand on its `link`, neither
naming the other.

- `link` (`pxl-gpu-link`, and `ts/gpu/link`, `@pxl/gpu/link`) is what both
  sides take: the stream's words, a handle's slot, a frame's admission and the
  program's settings (`GpuAsk`, `GpuConfig`), with the host's TypeScript
  bindings.
- `guest` (`pxl-gpu-guest`) is the guest's encoder of WebGPU's calls into the
  stream, with its bindings.
- `host` (`pxl-gpu-host`) runs the stream on wgpu on a device, with its
  bindings, served on the side `xpute-runtime-native` makes. It opens the
  device when the guest asks, keeps its compiled pipelines in the app's
  caches, and exports its C ABI by `native_abi!`, which pxl-runtime expands.
  It draws with a fork of wgpu ([wgpu-fork.md](wgpu-fork.md)).
- `ts/gpu/host` (`@pxl/gpu/host`) runs it on the browser's WebGPU in the
  page's worker (`WebGpu`), takes the canvas the UI gives beside the link,
  and posts its state to the page (`GpuReport`).
- `ts/gpu/expo` (`@pxl/gpu/expo`) is the device's module: the view the
  guest's surface draws into (`SurfaceView`), the display's frames, and what
  the host has of a GPU as it changes, the state now first to a UI that
  starts later (`state_on`).

The golden programs (`spec/golden/gpu/program.tsv`) hold the encoder and both
hosts to one record: the guest's test writes it on this machine's GPU through
the device's host, and the page's host is run on it in Chrome
(`pnpm --dir ts test:gpu:web`).

The stream is taken from a GPU's command buffer as WebGPU's encoder records
one.
