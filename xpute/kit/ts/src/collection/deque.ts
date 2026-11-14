// @xpute/kit/collection/deque.ts
// A deque is a double-ended queue in a ring over a buffer the caller owns. It
// never grows: a push onto a full deque is a bug (ENOSPC). A pop from an empty
// one returns undefined.

import type { u32 } from "@xpute/kit/abi/word.ts";
import { ERRNO_ENOSPC } from "@xpute/kit/status/errno.spec.ts";
import { bug } from "@xpute/kit/status/bug.ts";

export class Deque<T> {
  readonly buf: (T | undefined)[];
  head: u32;
  len: u32;

  // external storage (DOD-friendly): caller owns the ring buffer.
  constructor(buf: (T | undefined)[], head: u32 = 0, len: u32 = 0) {
    this.buf = buf;
    this.head = head;
    this.len = len;
  }

  cap(): u32 {
    return this.buf.length as u32;
  }

  is_empty(): boolean {
    return this.len === 0;
  }

  is_full(): boolean {
    return this.len === (this.buf.length as u32);
  }

  front(): T | undefined {
    return this.len ? (this.buf[this.head] as T) : undefined;
  }

  back(): T | undefined {
    if (!this.len) return undefined;
    const cap = this.cap();
    const i = (this.head + this.len - 1) % cap;
    return this.buf[i] as T;
  }

  push_back(v: T): void {
    if (this.is_full()) bug(ERRNO_ENOSPC);
    const cap = this.cap();
    const i = (this.head + this.len) % cap;
    this.buf[i] = v;
    this.len++;
  }

  push_front(v: T): void {
    if (this.is_full()) bug(ERRNO_ENOSPC);
    const cap = this.cap();
    this.head = (this.head + cap - 1) % cap;
    this.buf[this.head] = v;
    this.len++;
  }

  pop_front(): T | undefined {
    if (!this.len) return undefined;
    const v = this.buf[this.head] as T;
    this.buf[this.head] = undefined;
    this.head = (this.head + 1) % this.cap();
    this.len--;
    return v;
  }

  pop_back(): T | undefined {
    if (!this.len) return undefined;
    const cap = this.cap();
    const i = (this.head + this.len - 1) % cap;
    const v = this.buf[i] as T;
    this.buf[i] = undefined;
    this.len--;
    return v;
  }

  clear(): void {
    const cap = this.cap();
    for (let i = 0; i < this.len; i++) {
      this.buf[(this.head + i) % cap] = undefined;
    }
    this.head = 0;
    this.len = 0;
  }
}
