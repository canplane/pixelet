# What xpute answers for

**2026-11-07** — decided by the user. Main was at `f6048876bf`.

## What the user decided

- **xpute answers for what an MCU's chip does.** It stays a unikernel on a VM,
  its devices the runtime's software; the MCU says only how far it answers.
  - **Its devices are those that lend the core's memory across turns**, as an
    MCU's DMA peripherals are the chip's: the GPU (its stream and the
    surfaces a UI lends it) and the files (the bytes an `FS_WRITE` names are
    the runtime's until its `FS_CLOSE` is completed; an open and a close
    complete on a later turn, by their tag). Beside them, the memory and the
    core's time and turns.
  - **Everything else is the app's**, on one general channel: the app's ABI,
    the majors from `PROGRAM_MAJOR`, numbered messages whose meaning xpute
    does not know, as a UART or a mailbox. What passes only values, or writes
    the core's memory within one call, hangs there: text, sensors, sound,
    buttons. pixelet's text and sensors are its own.
  - **No device table.** The built-in devices are fixed.
- **The April checkpoint is followed** (`origin/checkpoint/20260405-rpc`): one
  manifest, `{ machine, core }`, read by the UI's `start` and a page's worker's
  `serve`; the imports the UI takes named by the UI (`cmds`); April's names
  where one was coined (`Link`, `on`). Its topology, device loader and `net`
  are not taken.
- **The ACKREQ contract stays**: without ACKREQ only a failure is answered, as
  io_uring's `IOSQE_CQE_SKIP_SUCCESS`.
- **A version only where data outlives the build**: a baked tile's format, a
  stored file's format, a tile set's version. The front's header holds none;
  the packages' version is said once, in xpute's Cargo workspace.
- **The generator that writes both ends of a protocol is xpute's**; an app owns
  its protocol files and where their output goes.
- **The UI's language is the shell's**, and reaches neither the runtime nor
  the core.
