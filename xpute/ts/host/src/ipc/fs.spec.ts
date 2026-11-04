// @xpute/host/ipc/fs.spec.ts
//
// GENERATED from spec/ipc/fs.json
//
// The file device's numbers (abi/fs.json, its protocol, Major FS): a guest's files,
// posted as frames on the completion ring for the device's port, which the host takes
// once the turn has returned and before the next begins. A frame may name memory the
// guest holds: the host reads or writes it in that gap, and the guest keeps it that long.
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. FS_OPENAT is the one call that waits: the host answers it with FS_OPENED once
// the file can be read whole. FS_READ and FS_WRITE are done in the gap; FS_CLOSE ends
// the descriptor, opened or not, and nothing is said of it after.

import * as flag from "./fs/flag.spec.ts";

/** How FS_OPENAT opens. */
export { flag };
