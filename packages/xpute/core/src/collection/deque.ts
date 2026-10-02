// @xpute/core/collection/deque.ts
// A deque over a caller-owned ring buffer. A pop from an empty deque is
// undefined; a push onto a full one overwrites the front and is the caller's bug.

import type { u32 } from "@xpute/core/abi/word.ts";
import { Errno } from "@xpute/core/status/errno.spec.ts";
import { InvariantError } from "@xpute/core/status/error.ts";

export class Deque<T> {
  constructor(
    public readonly buf: (T | undefined)[],
    public head: u32 = 0,
    public len: u32 = 0,
  ) {}

  cap(): u32 {
    return this.buf.length as u32;
  }

  size(): u32 {
    return this.len;
  }

  empty(): boolean {
    return this.len === 0;
  }

  full(): boolean {
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
    if (this.full()) throw new InvariantError(Errno.ENOSPC);
    const cap = this.cap();
    const i = (this.head + this.len) % cap;
    this.buf[i] = v;
    this.len++;
  }

  push_front(v: T): void {
    if (this.full()) throw new InvariantError(Errno.ENOSPC);
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
    for (let i = 0; i < this.len; i++) this.buf[(this.head + i) % cap] = undefined;
    this.head = 0;
    this.len = 0;
  }
}
