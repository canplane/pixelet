// @xpute/host/sched/strobe.ts

/**
 * The host's clock for one guest: a turn runs on an animation frame only when
 * the host (`request`) or the guest's last falling edge asked for one, never on
 * a timer of the host's own.
 */

import type { f64 } from "@xpute/kit/abi/word.ts";
import type { Doorbell } from "../ipc/doorbell.ts";
import { type Grant, Quantum, type QuantumPolicy } from "./quantum.ts";

export class Strobe {
  private readonly quantum: Quantum;
  private frame = false;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private timer_at = Infinity;
  private last = -1;
  private running = false;
  private readonly run = (now: f64): void => this.turn(now);

  /** `submit` fills the rings before the doorbell rings. */
  constructor(
    private readonly doorbell: Doorbell,
    policy: QuantumPolicy,
    private readonly submit: (grant: Grant, delta_s: f64) => void,
    private readonly next_frame: (run: (now: f64) => void) => void,
  ) {
    this.quantum = new Quantum(policy);
    doorbell.wake = (wake_ms) => this.wake(wake_ms);
  }

  start(): void {
    this.running = true;
    this.last = performance.now();
    this.request();
  }

  stop(): void {
    this.running = false;
    clearTimeout(this.timer);
    this.timer_at = Infinity;
  }

  request(): void {
    if (this.frame || !this.running) return;
    this.frame = true;
    this.next_frame(this.run);
  }

  input(): void {
    this.quantum.input(performance.now());
  }

  private wake(wake_ms: f64): void {
    if (!this.running) return;
    if (wake_ms === 0) return this.request();
    if (wake_ms < 0) return;
    const at = performance.now() + wake_ms;
    if (at >= this.timer_at) return;
    clearTimeout(this.timer);
    this.timer_at = at;
    this.timer = setTimeout(() => {
      this.timer_at = Infinity;
      this.request();
    }, wake_ms);
  }

  private turn(now: f64): void {
    this.frame = false;
    if (!this.running) return;
    // Unclamped: how long a gap still counts as motion is the guest's call,
    // and a clamp here is where every frame worth reporting would land.
    const delta_s = Math.max(0, (now - this.last) / 1000);
    this.last = now;
    clearTimeout(this.timer);
    this.timer_at = Infinity;
    const grant = this.quantum.grant(performance.now());
    this.submit(grant, delta_s);
    const t0 = performance.now();
    const wake_ms = this.doorbell.ring(grant.quota_ms);
    this.quantum.observe(delta_s, performance.now() - t0, grant, wake_ms);
  }
}
