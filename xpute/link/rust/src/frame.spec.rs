// xpute-link/frame.spec.rs
//
// GENERATED from link/spec/frame.json5 (xpute-gen data.rs)
//
// A frame is one method called across the link, four 32-bit words; TAG, CMD, RESULT and
// PKT are their indices, little-endian. The tag is the request, unique among the sender's
// pending ones, and a later answer carries it back. The command is the RES bit, the handle
// the callee issued above METHOD_BITS, and the method's place among its object's below
// them. An answer carries 0 or a negative errno, its value in its packet. The method's args, a packet in the Canonical ABI's layout,
// lie right past the words, and PKT says how long they are (link/wit/types.wit). ROOT
// is the handle every side implements, OPEN, CANCEL and OPEN_AT its methods, DROP the method
// every handle has, NAMED marks an interface's handle, SLOT_BITS splits an issued handle
// into its slot and its generation, and TRAP and TRAP_AT are a native guest's stop.

/// The request, which a later answer carries back.
pub const TAG: u32 = 0;

/// RES, the handle and the method.
pub const CMD: u32 = 1;

/// On an answer, 0 or a negative errno: the call refused, by the link or by the method as a caller's bug (EINVAL), or a method's failure where its WIT has no result; on a request, 0.
pub const RESULT: u32 = 2;

/// The length in bytes of the frame's packet, which lies right past its words; 0 when it has none.
pub const PKT: u32 = 3;

/// The frame's length in words.
pub const WORDS: u32 = 4;

/// The bit of the command that marks an answer.
pub const RES: u32 = 2147483648;

/// The bits of the command below the handle, which hold the method.
pub const METHOD_BITS: u32 = 8;

/// The root's handle, which every side implements.
pub const ROOT: u32 = 0;

/// The root's method that opens an object by its URI.
pub const OPEN: u32 = 0;

/// The root's method that cancels a call the sender has pending, named by its tag, the one
/// arg. A call the callee can stop it stops, and answers ECANCELED; any other runs on. Either
/// way it settles by its own answer, and what it lent stays lent until then.
pub const CANCEL: u32 = 1;

/// The root's method that opens an object under a handle the sender has issued for it:
/// the handle, which of the receiver's mounted openers opens it, by its place among them, and
/// the URI. A runtime that stands in for a UI on another thread answers the guest's open at
/// once, issuing the handle, and carries the open to the UI's end as this. It takes no
/// answer.
pub const OPEN_AT: u32 = 2;

/// The method every handle has, which lets it go.
pub const DROP: u32 = 255;

/// The handle's top bit, which marks it named: an interface's, its bits below the hash of
/// the interface's name the generator writes (xpute-link-gen wit.rs). An interface with
/// functions of its own is mounted under it with no open; one of resources alone names its
/// openers by it. An issued handle has it clear.
pub const NAMED: u32 = 4194304;

/// The bits of a handle that name its slot, below its generation.
pub const SLOT_BITS: u32 = 20;

/// What a native guest's export answers where the guest stopped, caught at the export so nothing leaves into its caller: as a module's trap. The guest is over, and its host enters it no more.
pub const TRAP: i32 = -2147483648;

/// What a native guest's link_init answers in its layout's place where the guest stopped, as TRAP is for its other exports: an odd address, where no layout lies.
pub const TRAP_AT: u64 = 4294967295;

/// The words of the link's layout, which a guest's link_init lays and answers the address
/// of: the inbox's address, a u64 in two words, its low word first, then a frame's cap, then a
/// word unused, so the inbox, which follows the layout's words, lies on 8 bytes. The outbox
/// follows the inbox.
pub mod layout {
    pub const INBOX_AT: usize = 0;
    pub const CAP_AT: usize = 2;
    pub const PAD_AT: usize = 3;
    pub const WORDS: usize = 4;
}
