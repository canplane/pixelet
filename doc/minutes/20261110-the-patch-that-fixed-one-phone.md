# The patch that fixed one phone

**2026-11-10** — recorded with the user. Main was at `576391033e` when this was
written. How one fault in wgpu was patched for the iPhone alone, how nothing
written anywhere said the Android phone had been left out, and what it took to
find the same fault there a day later. Times are Korean. The figures are in
`doc/experiment/20261110-one-command-buffer-an-encoder.md`; this is the account of
who did what and on what ground.

## A phone that died at boot

In the small hours of 2026-11-09 the iPhone app often died as it started. The
agent first took it for the memory limit, then said that was unproven, then at
04:48 read the kernel's own log, which settled it: the app passed its limit of 2348
MB two or three seconds after launch, every time, in the five minutes collected.

The user asked why 2.4 GiB, and why the phone and not the page, and asked for the
difference to be read from the code. For an hour the agent read and measured and
explained. It said the core, JavaScript and Metal sharing one process was the
reason, and withdrew that when the user pointed out that the app of late September
had shared one process too. It read every place the page and the app part in the
code and found none that allocates differently. It read wgpu and found no path that
makes gigabytes. It recorded the boot in Instruments and found no hidden gigabyte
in Metal's device allocation, which stopped at 801 MiB.

At 05:50 the user told it to stop running in every direction and look outside. It
searched, and within a minute had femtovg#367 and gfx-rs/wgpu#10506: wgpu's Metal
backend gives each pass its own `MTLCommandBuffer`, the driver holds about 2.3 MiB
for each until it completes, and an iPhone 12 had died at the same limit in the same
way. A frame of about 41 compute passes and 7 render passes made the sum. The search
did in a minute what the hour had not.

## What was read, what was built

The agent's report at 05:51 had the cause in the right place. It said wgpu-core
closes the command buffer and begins another each time a pass opens (`open_pass`),
and that the Metal backend's `begin_encoding` makes a new `MTLCommandBuffer` each
time. It recommended patching wgpu-core and wgpu-hal together so that passes stop
opening new command buffers.

The user gave it an hour. Fourteen minutes later it had built something narrower. It
vendored wgpu-hal alone and made the Metal backend share one command buffer among an
encoder's segments (`chain`), telling wgpu's internal segments apart by their
labels. wgpu-core still split every pass; the Metal backend now hid the split. That
is where the symptom was, and the phone that was dying was an iPhone.

It worked. Six launches lived past 25 seconds, where nearly all had died in three,
and a measuring run finished on the iPhone for the first time. The agent reported
the costs it knew of: indirect-call validation turned off, a deferred timestamp
resolve that now ran before its pass, and a patch keyed on wgpu's labels. It did not
report what the patch left alone, because it had not asked. No one opened
`vulkan/command.rs`. wgpu-core's split, which the agent had itself just named as the
start of the fault, is common to every backend.

## The sentence that closed the question

At 10:20 the user asked how the memory had shrunk. The agent explained the patch
and added that the web does not take this code and that Android has nothing to do
with it. For the web that was so. For Android it was a statement about a backend the
agent had not read and a memory it had not measured. In the same answer it was
careful about something smaller, saying that its 2.4 GB against 1.04 GB was not a
comparison under one condition.

The S23's memory was not looked at that day. The S23's trouble was frames and
touches, and those had causes of their own that were found and fixed.

Every record of the patch said Metal and stopped there:

- the commit `26880266d7`, one line on one Metal command buffer an encoder;
- `doc/tip/a-device-is-not-a-page.md`, a paragraph on Metal's command buffers;
- the comments in both `Cargo.toml`, naming `metal` and `chain`;
- `doc/experiment/20261109-the-app-that-did-not-move`, one line that the footprint
  without the Metal patch was not measured.

None said why only Metal, none said Vulkan had not been checked, and none said the
root lay in wgpu-core. So the next reader, the agent included, had nothing that
pointed at Android.

## A day later

On the night of 2026-11-09 to 10, the user asked whether 1.0 GiB on the iPhone was
normal, then had the iPhone and the S23 read under one condition. The iPhone stood
at 0.70 GiB and the S23 at 1.80 GiB, 1.38 of it GPU memory, with about the same
pixels on the screen. The agent said it did not know why and offered to read how
wgpu takes GPU memory on Android.

The user asked whether it had not done the hal work for the iPhone alone. The agent
confirmed that, then steered away from it: the Metal patch was about command buffers,
it said, and whether Vulkan had the same problem it had not yet looked at. It had
found another asymmetry in the code, the Vulkan allocator's large blocks, and it
offered that. The experiment showed the blocks were about 0.15 GiB. On the way, a
build failed and the script ran on the previous APK; those figures were thrown away.
The agent then reported that the difference was mostly somewhere else, that it did
not know where, and asked whether to go on or stop there.

The user answered that it had been told to get to the bottom of things, that it had
not looked at outside sources, that the Metal fault too had been found only because
the user had told it to search, and asked why searching came so hard to it.

The first search that named the right words found it. A comment of 2026-10-17 on
gfx-rs/wgpu#5738, against wgpu 30.0.1 on an Adreno 840: `Graphics` memory going from
about 120 MB to about 2.1 GB and staying there, because wgpu-core makes one hal
command buffer for each pass and another for its barriers, and wgpu-hal resets its
Vulkan pools without `RELEASE_RESOURCES`, so each pool keeps the memory of its
heaviest frame. It was the fault patched for Metal two nights before, in the place
named that night, showing on the other phone. wgpu's allocator report agreed that
the memory was not in the buffers and textures: 704 MiB reserved against a driver
count of 1,337 MiB.

## One path

The agent began to confirm the cause by resetting the pools with
`RELEASE_RESOURCES`. The user stopped it and told it to make the patch and apply it,
and asked whether this was not the violation of symmetry it had looked like all
along. It was.

The comment on #5738 links a prototype against the same version. It records a pass
on the CPU, encodes the pass's barriers into the encoder's open command buffer, and
replays the pass after them, so that an encoder is one hal command buffer on every
backend. The agent vendored wgpu-core, applied the prototype, put wgpu-hal's Metal
back to upstream but for the simulator's capabilities, and stopped turning off
`DISCARD_HAL_LABELS`, which only the labels of `chain` had needed. Both workspaces
resolve to the vendored crates. `xpute/rust/Cargo.lock`, which is ignored by git,
had been read before cargo had run in that workspace, so it still showed the
registry until it had. The checks, the GPU tests and xpute's tests passed.

Under one condition, before and after, memory fell on both phones and GPU time and
frames drawn did not change. The patch is common, and the effect shows on both.

## What the fault was

The fault in wgpu was reported, public, and two searches away. The fault in the work
was smaller and has a shape. The cause was read correctly and then the fix was put
where the symptom was, under a deadline, on the one platform that was failing. Then
a sentence about the other platform was said without being checked, and then the
records wrote down what was done and not what was skipped. The same night the same
shape appeared in the tile fetch, where a client chosen on one platform lost a cache
the other kept, and was first answered by a fix for the one platform.

The user's position, stated more than once over those two days: a difference between
the platforms is not a thing to be explained afterwards. Where the fault is common,
the fix is common. And what is known from outside is to be looked for before an hour
is spent on the inside.

## Left as it is

The user decided the prototype stays as it was taken. Its recorder extends borrows
to `'static` with a transmute, held sound by wgpu-core's trackers, and its author
does not offer it for upstream as it is. Doing it properly would be two walks over a
pass in wgpu-core, which the agent said it was not confident of, and which would
take the recorder and the transmute away.

Not known: the recorder's CPU cost on the GPU's thread, what the memory above the
allocator's blocks holds after the patch, and whether the timestamp order the `chain`
changed has gone with it. The S23's `Graphics` still stands well above the iPhone's
footprint, on two measures that do not compare exactly.
