// @xpute/host/ipc/sys.spec.ts
//
// GENERATED from spec/ipc/sys.json
//
// A guest's hostcalls, its calls into its host: its files, the report that stopped it and
// the program's own calls, as records in a stream (ipc/stream.rs) the host runs once each
// turn has returned and before the next begins. A record may name memory the guest holds:
// the host reads or writes it in that gap, and the guest keeps it that long. The toc
// says where the stream is (key SYS).
//
// Files borrow Plan 9's file namespace, in POSIX's words. The guest issues the descriptor,
// a generational handle, and names a file by a root the program numbers and a path below
// it; the program binds each root to where it keeps things, and a path that leaves it is
// refused. OPENAT is the one call that waits: the host answers it with the command
// SYS_OPENED (abi/cmd.json) once the file can be read whole. READ and WRITE are done in
// the gap; CLOSE ends the descriptor, opened or not, and nothing is said of it after.
//
// A program's own calls go in the same stream, numbered with the PROGRAM bit, and a host
// that is not the program's refuses them.
//
// A program's hostcall the host answers from what it already holds may be run now instead:
// the same record, laid past the last the turn has recorded and never published, its offset
// handed to the host's `hostcall` entry, which runs it before it returns and answers
// with a value or -errno. It writes only what the record names, and ENOSYS where the call
// is not the program's or no program answers. xpute's own calls go in the stream alone.

import type { u32 } from "@xpute/kit/abi/word.ts";
import * as op from "./sys/op.spec.ts";
import * as args from "./sys/args.spec.ts";
import * as abort from "./sys/abort.spec.ts";
import * as flag from "./sys/flag.spec.ts";

/** Words before the first record: END, then a word kept at 0. */
export const HEAD: u32 = 2 as number;

/** A record's call, then its arguments. */
export { op };

/** How many words of arguments each call's record takes, as its `op` lists them; OPENAT's
 * and ABORT's end in bytes after them. */
export { args };

/** Where ABORT's two values lie among its arguments, each low word then high. */
export { abort };

/** How OPENAT opens. */
export { flag };
