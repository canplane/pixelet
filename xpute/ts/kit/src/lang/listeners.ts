// @xpute/kit/lang/listeners.ts

/** Functions to call when something changes. `watch` adds one and returns how to remove it. */
export class Listeners {
  readonly #listeners = new Set<() => void>();

  readonly watch = (listener: () => void): (() => void) => {
    this.#listeners.add(listener);
    return () => this.#listeners.delete(listener);
  };

  notify(): void {
    for (const listener of this.#listeners) listener();
  }
}
