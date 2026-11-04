// @xpute/host/ipc/sys.spec.ts
//
// GENERATED from spec/ipc/sys.json
//
// A guest's hostcalls, its calls into its host: its files and the report that stopped it.
// Each is a frame for the host's port, 0, on the completion ring (abi/cmd.json, Major SYS),
// which the host takes once the turn has returned and before the next begins. A frame may
// name memory the guest holds: the host reads or writes it in that gap, and the guest
// keeps it that long.
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. SYS_OPENAT is the one call that waits: the host answers it with SYS_OPENED once
// the file can be read whole. SYS_READ and SYS_WRITE are done in the gap; SYS_CLOSE ends
// the descriptor, opened or not, and nothing is said of it after. SYS_ABORT is the report
// a guest makes as it stops, the one frame of its last turn.
//
// A call answered before it returns is a frame too, to any port: laid in the completion
// ring's next slot, not published, its position handed to the host's `hostcall` entry,
// which hands it to the port it names and returns that port's answer, a value or -errno.

import * as flag from "./sys/flag.spec.ts";

/** How SYS_OPENAT opens. */
export { flag };
