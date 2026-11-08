// @xpute/runtime/ipc/ring.spec.ts
//
// GENERATED from spec/ipc/ring.json
//
// A ring of frames (frame.json) with one producer and one consumer, in memory both sides
// can see. Each side names the ring by its offset in that memory, so the name means the
// same to both. The ring begins with a header of HEADER_WORDS 32-bit words, and every other
// name here is the index of one header word. The frames follow the header; their payloads
// lie apart in slots, as in virtio and AF_XDP. Position n owns frame n modulo the capacity
// and the slot of the same index, so a slot is free exactly when its frame is. Positions
// only count up, and each side writes only its own position word, so the ring needs no lock.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** How many frames the ring holds, a power of two. */
export const CAP: u32 = 0 as number;

/** The consumer's position: the next frame it reads. */
export const HEAD: u32 = 1 as number;

/** The producer's position: where the next frame goes. */
export const TAIL: u32 = 2 as number;

/** The size of one payload slot in bytes, a power of two and a multiple of 8. */
export const SLOT_BYTES: u32 = 3 as number;

/** The offset in the memory where the slots start. */
export const SLOT_BASE: u32 = 4 as number;

/** Reserved; it holds 0. */
export const RESERVED_0: u32 = 5 as number;

/** Reserved; it holds 0. */
export const RESERVED_1: u32 = 6 as number;

/** Reserved; it holds 0. */
export const RESERVED_2: u32 = 7 as number;

/** The header's length in words; the first frame follows it. */
export const HEADER_WORDS: u32 = 8 as number;
