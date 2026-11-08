// @xpute/kit/lang/listener.ts

/**
 * These are functions to call when something changes. `watch` adds one and
 * returns a function that removes it.
 */
export class ListenerSet {
  private readonly _listeners = new Set<() => void>();

  readonly watch = (listener: () => void): (() => void) => {
    this._listeners.add(listener);
    return () => this._listeners.delete(listener);
  };

  notify(): void {
    for (const listener of this._listeners) listener();
  }
}
