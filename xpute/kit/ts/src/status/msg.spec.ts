// @xpute/kit/status/msg.spec.ts
//
// GENERATED from kit/spec/msg.json5 (xpute-gen data.rs)
//
// A message is what a person reads of what xpute tells: of an error, beside its errno and its
// place; of a bug, beside those and its two numbers; of a log record, beside its level and
// its context. It is bounded text: its maker cuts a longer one at a character boundary
// (status/msg `cut_msg`), so nothing that carries one meets one past the cap. The text told
// beside it, a place's file and a record's context, is held to the same cap by the same cut.

/** The most bytes of UTF-8 a message holds. */
export const MSG_CAP = 512 as number;
