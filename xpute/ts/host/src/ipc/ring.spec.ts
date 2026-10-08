// @xpute/host/ipc/ring.spec.ts
//
// GENERATED from spec/ipc/ring.json — do not edit.
//
// A single-producer, single-consumer ring of frames (frame.json) in memory both sides
// see, named by its offset so it means the same to either. A header, then a frame a
// message; the payloads lie apart in slots, as in virtio and AF_XDP. Message n owns
// frame and slot n modulo the capacity, so a slot is free exactly when its frame is.
// Positions only grow, and each side writes only its own word, so no lock is needed.

import type { u32 } from "@xpute/kit/abi/word.ts";

/** How many frames, a power of two. */
export const CAPACITY: u32 = 0 as number;

/** The consumer's position: the next message it reads. */
export const HEAD: u32 = 1 as number;

/** The producer's position: where the next message goes. */
export const TAIL: u32 = 2 as number;

/** A payload slot's size, a power of two and a multiple of 8. */
export const SLOT_BYTES: u32 = 3 as number;

/** Where the slots start, as an offset in the memory. */
export const SLOT_BASE: u32 = 4 as number;

/** Kept at 0. */
export const RESERVED_0: u32 = 5 as number;

/** Kept at 0. */
export const RESERVED_1: u32 = 6 as number;

/** Kept at 0. */
export const RESERVED_2: u32 = 7 as number;

/** Words before the first frame. */
export const HEADER_WORDS: u32 = 8 as number;
