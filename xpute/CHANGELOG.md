# Changes

What each export to xpute's own repository changed, newest first, for a program
that took an earlier one. Each says first what changed in the **contract**, the
formats and calls the two ends keep with each other (ARCHITECTURE.md, "The
contract"), so a guest or a host taken from an earlier export knows whether it
still talks to the other end; then what changed in **the ends' own calls**,
which only a program calling them follows. A change marked **breaking** is one
an end taken earlier cannot meet without changing.

The entry not yet exported is headed by the repository's last commit; the
export heads it with its own date and commit.

Exports before the first entry are told by the repository's commit messages.

## Since 895b98e35, 2026-10-26

### The contract

- **Breaking.** The entry is `run`, named as `KVM_RUN` is: a module exports
  `run` and a guest linked into a native program `xpute_run`, where both were
  `intr`.
- **Breaking, for a host.** A guest may call its host by **hostcall**, a call of
  its program's the host answers before it returns (rule 8). A module imports
  it as `env.hostcall`, so a host that does not provide that import cannot
  instantiate one; a guest linked into a native program exports
  `xpute_hostcall_on`, and the guest is handed it with its clock (`BootInfo`).
  A host with nothing to answer answers `ENOSYS`.
- **Breaking, for a host.** The directory's keys 3, 4 and 5, the frame pacing's
  numbers, are retired, with `QUANTUM_SCALE`: the scheduling policy is the
  host's, injected when it boots. A host passes over a key it does not know
  (rule 0), so a guest that still writes them boots; a host that still reads
  them refuses a guest that does not.
- Offset 0 names nothing, so nothing that crosses lies there (rule 1), as a
  frame with no packet already said.
- Names only, their numbers kept: the frame's packet word is `PKT`, and a
  completion without `RES` is an **event**, where both were `PACKET` and a
  signal.

### The ends' own calls

- The host is mounted with a scheduling policy (`@xpute/host/sched/policy.ts`
  `SchedulingPolicy`) and what it submits on each turn granted; frame pacing
  (`sched/frame_pacing.ts` `FramePacing`) and a fixed quota for a host with no
  display (`sched/fixed_quota.ts` `FixedQuota`) are two, and the strobe
  (`Strobe`) is the mechanism they steer. `Quantum` is gone.
- A module is opened over its memory in one call (`@xpute/host/module.ts`
  `open_module`), and every part of the program's answers (`SysHost`) is
  optional: a file system left out refuses every open, a hostcall with no
  program `ENOSYS`.
- The host's doorbell hears events through `event`, and the guest's door is
  told the most one command raises as `most_events`, where they were `signal`
  and `most_signals`.
- A tick's phases are the program's: it numbers them (`Phase`, at most
  `PHASES_MAX`) and gives each the share held back for the ones after it
  (`TickPolicy` `held_back`); `spec/sched/tick.json` is gone.
- The smallest guest and the host that drives it are `rust/example` and
  `ts/example`, and the contract's traces run under eight layouts each guest
  draws from a seed (`spec/conformance/guest.json` `LAYOUTS`).
